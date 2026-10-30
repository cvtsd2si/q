use std::collections::{BTreeMap, BTreeSet};

use crate::asm::r#gen::{AsmOp, AsmReg};
use crate::asm::regalloc::{AsmLocation, RegClass};
use crate::{
    analysis::liveness::{Liveness, LivenessInstruction},
    asm::r#gen::AsmInstr,
};

const GP_REGS: &[AsmReg] = &[AsmReg::R8, AsmReg::R11];

const SSE_REGS: &[AsmReg] = &[
    AsmReg::Xmm0,
    AsmReg::Xmm1,
    AsmReg::Xmm2,
    AsmReg::Xmm3,
    AsmReg::Xmm4,
    AsmReg::Xmm5,
    AsmReg::Xmm6,
    AsmReg::Xmm7,
];

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum InterferenceNode {
    Pseudo(String),
    Reg(AsmReg),
}

impl InterferenceNode {
    fn from_location(location: &AsmLocation) -> Option<Self> {
        match location {
            AsmLocation::Pseudo(name) => Some(Self::Pseudo(name.clone())),
            AsmLocation::Reg(reg) => Some(Self::Reg(*reg)),
            AsmLocation::Flags => None,
        }
    }

    fn is_precolored(&self) -> bool {
        matches!(self, Self::Reg(_))
    }
}

#[derive(Debug, Default)]
pub struct InterferenceGraph {
    pub neighbors: BTreeMap<InterferenceNode, BTreeSet<InterferenceNode>>,
}

impl InterferenceGraph {
    pub fn build(instructions: &[AsmInstr], liveness: &Liveness<AsmLocation>) -> Self {
        assert_eq!(instructions.len(), liveness.instrs.len());

        let mut graph = Self::default();

        for instr in instructions {
            let ud = instr.use_def();

            for location in ud.uses.iter().chain(ud.defs.iter()) {
                if let Some(node) = InterferenceNode::from_location(location) {
                    graph.add_node(&node);
                }
            }
        }

        for (idx, instr) in instructions.iter().enumerate() {
            let ud = instr.use_def();

            let copy_source = match instr {
                AsmInstr::Mov {
                    src: AsmOp::Pseudo { name, .. },
                    ..
                } => Some(InterferenceNode::Pseudo(name.clone())),

                AsmInstr::Mov {
                    src: AsmOp::Reg(reg),
                    ..
                } => Some(InterferenceNode::Reg(*reg)),

                _ => None,
            };

            for def in &ud.defs {
                let Some(def_node) = InterferenceNode::from_location(def) else {
                    continue;
                };

                for live in &liveness.instrs[idx].live_out {
                    let Some(live_node) = InterferenceNode::from_location(live) else {
                        continue;
                    };

                    if copy_source.as_ref() == Some(&live_node) {
                        continue;
                    }

                    graph.add_edge(&def_node, &live_node);
                }
            }
        }

        // Registers simultaneously live at a block's entry
        // must occupy distinct physical locations.
        for block in &liveness.blocks {
            let live: Vec<_> = block
                .live_in
                .iter()
                .filter_map(InterferenceNode::from_location)
                .collect();

            // Ensure even isolated live-in values exist in the graph.
            for node in &live {
                graph.add_node(node);
            }

            // Every pair of simultaneously live values interferes.
            for i in 0..live.len() {
                for j in (i + 1)..live.len() {
                    graph.add_edge(&live[i], &live[j]);
                }
            }
        }

        graph
    }

    pub fn color(
        &self,
        classes: &BTreeMap<String, RegClass>,
        spill_costs: &BTreeMap<String, usize>,
    ) -> (BTreeMap<String, AsmReg>, BTreeSet<String>) {
        let stack = self.simplify(classes, spill_costs);

        let mut assigned = BTreeMap::new();
        let mut spilled = BTreeSet::new();

        for node in stack.into_iter().rev() {
            let InterferenceNode::Pseudo(name) = &node else {
                unreachable!("Simplification must only return pseudos");
            };

            let class = *classes
                .get(name)
                .expect("Missing register class for pseudo");

            let candidates = registers_for(class);

            let mut blocked = BTreeSet::new();

            for neighbor in &self.neighbors[&node] {
                match neighbor {
                    InterferenceNode::Reg(reg) => {
                        blocked.insert(*reg);
                    }

                    InterferenceNode::Pseudo(other) => {
                        if let Some(&reg) = assigned.get(other) {
                            blocked.insert(reg);
                        }
                    }
                }
            }

            if let Some(reg) = candidates
                .iter()
                .copied()
                .find(|reg| !blocked.contains(reg))
            {
                assigned.insert(name.clone(), reg);
            } else {
                spilled.insert(name.clone());
            }
        }

        (assigned, spilled)
    }

    pub fn simplify(
        &self,
        classes: &BTreeMap<String, RegClass>,
        spill_costs: &BTreeMap<String, usize>,
    ) -> Vec<InterferenceNode> {
        let mut working = self.neighbors.clone();
        let mut stack = Vec::new();

        let spill_score = |node: &InterferenceNode, neighbors: &BTreeSet<InterferenceNode>| {
            let InterferenceNode::Pseudo(name) = node else {
                unreachable!("Physical registers cannot spill");
            };

            let cost = spill_costs.get(name).copied().unwrap_or(0);

            let degree = relevant_degree(node, neighbors, classes).max(1);

            cost as f64 / degree as f64
        };

        while working.keys().any(|node| !node.is_precolored()) {
            // First, look for a node whose degree is smaller
            // than the number of registers in its class.
            let candidate = working
                .iter()
                .filter(|(node, _)| !node.is_precolored())
                .find(|(node, neighbors)| {
                    let InterferenceNode::Pseudo(name) = node else {
                        unreachable!();
                    };

                    let class = *classes.get(name).expect("Missing register class");

                    let k = registers_for(class).len();

                    let degree = relevant_degree(node, neighbors, classes);

                    degree < k
                })
                .map(|(node, _)| node.clone())
                .or_else(|| {
                    working
                        .iter()
                        .filter(|(node, _)| !node.is_precolored())
                        .min_by(|(a, a_neighbors), (b, b_neighbors)| {
                            let score_a = spill_score(a, a_neighbors);
                            let score_b = spill_score(b, b_neighbors);

                            score_a.total_cmp(&score_b)
                        })
                        .map(|(node, _)| node.clone())
                })
                .expect("At least one pseudo-register must remain");

            // Remove this pseudo from the working graph.
            let neighbors = working.remove(&candidate).unwrap();

            // Remove its edges from remaining nodes.
            for neighbor in neighbors {
                if let Some(adjacent) = working.get_mut(&neighbor) {
                    adjacent.remove(&candidate);
                }
            }

            // Record removal order for the coloring phase.
            stack.push(candidate);
        }

        stack
    }

    pub fn merge_nodes(&mut self, keep: &InterferenceNode, remove: &InterferenceNode) {
        assert_ne!(keep, remove);

        assert!(
            matches!(remove, InterferenceNode::Pseudo(_)),
            "Cannot remove a physical register"
        );

        let keep_neighbors = self.neighbors.get(keep).unwrap();

        assert!(
            !keep_neighbors.contains(remove),
            "Cannot merge interfering nodes"
        );

        let removed_neighbors = self.neighbors.remove(remove).unwrap();

        for neighbor in &removed_neighbors {
            let adjacent = self.neighbors.get_mut(neighbor).unwrap();

            adjacent.remove(remove);
            adjacent.insert(keep.clone());
        }

        self.neighbors
            .get_mut(keep)
            .unwrap()
            .extend(removed_neighbors);
    }

    fn add_node(&mut self, node: &InterferenceNode) {
        self.neighbors.entry(node.to_owned()).or_default();
    }

    fn add_edge(&mut self, a: &InterferenceNode, b: &InterferenceNode) {
        if a == b {
            return;
        }

        self.neighbors
            .entry(a.to_owned())
            .or_default()
            .insert(b.to_owned());

        self.neighbors
            .entry(b.to_owned())
            .or_default()
            .insert(a.to_owned());
    }
}

pub(super) fn registers_for(class: RegClass) -> &'static [AsmReg] {
    match class {
        RegClass::Gp => GP_REGS,
        RegClass::Sse => SSE_REGS,
    }
}

pub(super) fn relevant_degree(
    node: &InterferenceNode,
    neighbors: &BTreeSet<InterferenceNode>,
    classes: &BTreeMap<String, RegClass>,
) -> usize {
    let InterferenceNode::Pseudo(name) = node else {
        return 0;
    };

    let class = *classes.get(name).expect("Missing register class");

    let candidates = registers_for(class);

    neighbors
        .iter()
        .filter(|neighbor| match neighbor {
            InterferenceNode::Pseudo(other) => classes.get(other) == Some(&class),

            InterferenceNode::Reg(reg) => candidates.contains(reg),
        })
        .count()
}
