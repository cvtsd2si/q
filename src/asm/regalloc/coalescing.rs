use std::collections::{BTreeMap, BTreeSet};

use crate::asm::{
    r#gen::{AsmInstr, AsmOp, AsmReg},
    regalloc::interference::relevant_degree,
};

use super::{
    RegClass,
    interference::{InterferenceGraph, InterferenceNode, registers_for},
};

#[derive(Debug, Clone)]
pub struct Move {
    pub src: String,
    pub dst: String,
}

pub struct Coalescing {
    pub graph: InterferenceGraph,

    // Original pseudo name -> surviving graph node.
    pub representative: BTreeMap<String, InterferenceNode>,
}

impl Coalescing {
    pub fn new(graph: InterferenceGraph) -> Self {
        let representative = graph
            .neighbors
            .keys()
            .filter_map(|node| {
                let InterferenceNode::Pseudo(name) = node else {
                    return None;
                };

                Some((name.clone(), node.clone()))
            })
            .collect();

        Self {
            graph,
            representative,
        }
    }

    pub fn try_coalesce(&mut self, mov: &Move, classes: &BTreeMap<String, RegClass>) -> bool {
        // Only merge pseudos belonging to the same register class.
        let Some(&class) = classes.get(&mov.src) else {
            return false;
        };

        if classes.get(&mov.dst) != Some(&class) {
            return false;
        }

        // Resolve previously merged nodes.
        let Some(src) = self.representative.get(&mov.src).cloned() else {
            return false;
        };

        let Some(dst) = self.representative.get(&mov.dst).cloned() else {
            return false;
        };

        // Both original names already refer to the same node.
        if src == dst {
            return true;
        }

        // Choose the coalescing criterion based on the
        // CURRENT representatives, not the original names.
        match (&src, &dst) {
            // Source is now precolored.
            // Try merging the destination into that register.
            (InterferenceNode::Reg(reg), InterferenceNode::Pseudo(_)) => {
                return self.try_coalesce_precolored(&mov.dst, *reg, classes);
            }

            // Destination is now precolored.
            // Try merging the source into that register.
            (InterferenceNode::Pseudo(_), InterferenceNode::Reg(reg)) => {
                return self.try_coalesce_precolored(&mov.src, *reg, classes);
            }

            // We cannot merge distinct physical registers.
            (InterferenceNode::Reg(_), InterferenceNode::Reg(_)) => {
                return false;
            }

            // Both representatives are still pseudos.
            // Continue with the existing Briggs check below.
            (InterferenceNode::Pseudo(_), InterferenceNode::Pseudo(_)) => {}
        }

        let Some(src_neighbors) = self.graph.neighbors.get(&src) else {
            return false;
        };

        let Some(dst_neighbors) = self.graph.neighbors.get(&dst) else {
            return false;
        };

        // Interfering nodes cannot share a register.
        if src_neighbors.contains(&dst) {
            return false;
        }

        // The merged node inherits both sets of neighbors.
        let mut merged_neighbors: BTreeSet<_> =
            src_neighbors.union(dst_neighbors).cloned().collect();

        merged_neighbors.remove(&src);
        merged_neighbors.remove(&dst);

        let registers = registers_for(class);

        let k = registers.len();

        // Briggs' criterion:
        //
        // The merged node must have fewer than K
        // significant-degree neighbors.
        let significant = merged_neighbors
            .iter()
            .filter(|neighbor| match neighbor {
                InterferenceNode::Reg(reg) => {
                    // Precolored registers cannot be simplified.
                    // Count them if they belong to this register pool.
                    registers.contains(reg)
                }

                InterferenceNode::Pseudo(name) => {
                    // Different register classes do not compete.
                    if classes.get(name) != Some(&class) {
                        return false;
                    }

                    let neighbors = self
                        .graph
                        .neighbors
                        .get(*neighbor)
                        .expect("Missing interference node");

                    // A pseudo is significant if its relevant
                    // degree is at least K.
                    relevant_degree(neighbor, neighbors, classes) >= k
                }
            })
            .count();

        // Reject merges that fail Briggs' criterion.
        if significant >= k {
            return false;
        }

        // Keep destination, remove source.
        self.graph.merge_nodes(&dst, &src);

        // Redirect all original pseudos represented by src.
        for representative in self.representative.values_mut() {
            if *representative == src {
                *representative = dst.clone();
            }
        }

        true
    }

    pub fn try_coalesce_precolored(
        &mut self,
        name: &str,
        reg: AsmReg,
        classes: &BTreeMap<String, RegClass>,
    ) -> bool {
        let Some(current) = self.representative.get(name).cloned() else {
            return false;
        };

        let fixed = InterferenceNode::Reg(reg);

        // Already assigned to this physical register.
        if current == fixed {
            return true;
        }

        // Don't attempt to merge two different physical registers.
        if !matches!(&current, InterferenceNode::Pseudo(_)) {
            return false;
        }

        if !can_coalesce_george(&self.graph, &current, reg, classes) {
            return false;
        }

        // Keep the precolored node and remove the pseudo.
        self.graph.merge_nodes(&fixed, &current);

        // Redirect all aliases of the removed pseudo.
        for representative in self.representative.values_mut() {
            if *representative == current {
                *representative = fixed.clone();
            }
        }

        true
    }
}

pub fn collect_moves(instructions: &[AsmInstr]) -> Vec<Move> {
    let mut moves = Vec::new();

    for instr in instructions {
        if let AsmInstr::Mov {
            src:
                AsmOp::Pseudo {
                    name: src,
                    ty: src_ty,
                },
            dst:
                AsmOp::Pseudo {
                    name: dst,
                    ty: dst_ty,
                },
            ..
        } = instr
            && src != dst
            && RegClass::of_type(src_ty) == RegClass::of_type(dst_ty)
        {
            moves.push(Move {
                src: src.clone(),
                dst: dst.clone(),
            });
        }
    }

    moves
}

fn can_coalesce_george(
    graph: &InterferenceGraph,
    pseudo: &InterferenceNode,
    reg: AsmReg,
    classes: &BTreeMap<String, RegClass>,
) -> bool {
    let InterferenceNode::Pseudo(name) = pseudo else {
        return false;
    };

    let Some(&class) = classes.get(name) else {
        return false;
    };

    let registers = registers_for(class);

    // For now, only coalesce with registers belonging
    // to our allocation pool.
    if !registers.contains(&reg) {
        return false;
    }

    let fixed = InterferenceNode::Reg(reg);

    let Some(pseudo_neighbors) = graph.neighbors.get(pseudo) else {
        return false;
    };

    let Some(fixed_neighbors) = graph.neighbors.get(&fixed) else {
        return false;
    };

    // Direct interference makes coalescing impossible.
    if pseudo_neighbors.contains(&fixed) {
        return false;
    }

    let k = registers.len();

    // George's criterion:
    // Every neighbor of the pseudo must be insignificant,
    // precolored, or already interfere with the target.
    pseudo_neighbors.iter().all(|neighbor| {
        match neighbor {
            InterferenceNode::Reg(_) => {
                // Physical registers are already colored.
                true
            }

            InterferenceNode::Pseudo(other) if classes.get(other) != Some(&class) => {
                // Different register classes don't compete.
                true
            }

            InterferenceNode::Pseudo(_) => {
                // The neighbor already interferes with
                // the target register.
                if fixed_neighbors.contains(neighbor) {
                    return true;
                }

                // Otherwise it must be low-degree.
                graph
                    .neighbors
                    .get(neighbor)
                    .is_some_and(|neighbors| relevant_degree(neighbor, neighbors, classes) < k)
            }
        }
    })
}

pub fn collect_precolored_moves(instructions: &[AsmInstr]) -> Vec<(String, AsmReg)> {
    let mut moves = Vec::new();

    for instr in instructions {
        match instr {
            // mov pseudo, physical_register
            AsmInstr::Mov {
                src: AsmOp::Pseudo { name, .. },
                dst: AsmOp::Reg(reg),
                ..
            }
            // mov physical_register, pseudo
            | AsmInstr::Mov {
                src: AsmOp::Reg(reg),
                dst: AsmOp::Pseudo { name, .. },
                ..
            } => {
                moves.push((name.clone(), *reg));
            }

            _ => {}
        }
    }

    moves
}
