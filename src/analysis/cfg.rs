use std::{
    collections::{BTreeSet, HashMap},
    ops::Range,
};

use crate::{
    asm::r#gen::AsmInstr,
    ir::r#gen::{IRFunc, IRInstr},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockId(pub usize);

#[derive(Debug, Clone)]
pub struct BasicBlock {
    pub range: Range<usize>,
    pub preds: Vec<BlockId>,
    pub succs: Vec<BlockId>,
}

#[derive(Debug)]
pub struct Cfg {
    pub blocks: Vec<BasicBlock>,
    /* Map from IRInstr idx to its containing basic block.  */
    pub instr_to_block: Vec<BlockId>,
}

impl Cfg {
    pub fn build<I>(instructions: &[I]) -> Result<Self, String>
    where
        I: CfgInstr,
    {
        if instructions.is_empty() {
            return Ok(Self {
                blocks: Vec::new(),
                instr_to_block: Vec::new(),
            });
        }

        /* Analyze the fn body and figure out the body layout,
         * that is, where each basic block begins and ends.  */
        let body_layout = analyze_body(instructions)?;

        let leaders: Vec<usize> = body_layout.leaders.into_iter().collect();

        /* Construct basic blocks.  */
        let mut blocks = Vec::new();
        let mut instr_to_block = vec![BlockId(0); instructions.len()];

        for (block_id, &start) in leaders.iter().enumerate() {
            /* The end of the current block is where the next leader begins,
             * or the end of the function if there are no more leaders.  */
            let end = leaders
                .get(block_id + 1)
                .copied()
                .unwrap_or(instructions.len());

            for item in instr_to_block.iter_mut().take(end).skip(start) {
                *item = BlockId(block_id);
            }

            blocks.push(BasicBlock {
                range: start..end,
                preds: Vec::new(),
                succs: Vec::new(),
            });
        }

        /* Resolve labels to block ids.  */
        let resolve_label = |name: &str| -> Result<BlockId, String> {
            let &instr_idx = body_layout
                .labels
                .get(name)
                .ok_or_else(|| format!("Unknown label: {name}"))?;

            Ok(instr_to_block[instr_idx])
        };

        /* Construct successor edges.  */
        for id in 0..blocks.len() {
            let last_idx = blocks[id].range.end - 1;
            let last_instr = &instructions[last_idx];

            /* If there exists another block after the current block, return its BlockId,
             * or else return None.  */
            let next = (id + 1 < blocks.len()).then_some(BlockId(id + 1));

            let mut succs = Vec::new();

            /* The last instruction in a basic block determines its successors.  */
            match last_instr.cfg_flow() {
                /* For an unconditional jump, its successor is the jump's target.  */
                CfgFlow::Jump(target) => {
                    succs.push(resolve_label(target)?);
                }

                /* A conditional jump has two successors:
                 *  1. the jump target, if the condition checks out,
                 *  2. the following basic block, if the condition does not hold
                 *     and the execution falls through.  */
                CfgFlow::Branch(target) => {
                    succs.push(resolve_label(target)?);

                    /* Handle the case where the target is the fallthrough block itself.  */
                    if let Some(next) = next
                        && !succs.contains(&next)
                    {
                        succs.push(next);
                    }
                }

                /* Returns have no successors.  */
                CfgFlow::Return => {}

                /* Otherwise the successor is the fallthrough block.  */
                CfgFlow::Next | CfgFlow::Label(_) => {
                    if let Some(next) = next {
                        succs.push(next);
                    }
                }
            }

            blocks[id].succs = succs;
        }

        /* Construct predecessor edges.  */
        for id in 0..blocks.len() {
            let successors = blocks[id].succs.clone();

            for successor in successors {
                blocks[successor.0].preds.push(BlockId(id));
            }
        }

        Ok(Self {
            blocks,
            instr_to_block,
        })
    }

    pub fn to_dot(&self, func: &IRFunc) -> String {
        /* The `graph` Graphviz's directive tells Graphviz how to lay out the graph.
         *
         * TB = top to bottom
         * shape=box means the nodes should be rectangular.  */
        let mut dot = String::from(
            "digraph CFG {\n\
             graph [rankdir=TB];\n\
             node [shape=box, fontname=\"monospace\"];\n",
        );

        /* Create one node for each basic block.  */
        for (id, block) in self.blocks.iter().enumerate() {
            /* `\\l` produces Graphviz's `\l` escape sequence, which creates a new line with
             * left-aligned text.  */
            let mut label = format!("B{id} [{}..{})\\l", block.range.start, block.range.end);

            /* Add instructions to the node's label.  */
            for idx in block.range.clone() {
                let instr = &func.body[idx];

                label.push_str(&self.escape_dot(&format!("{idx}: {instr:?}")));

                label.push_str("\\l");
            }

            /* Add node to graph.  */
            dot.push_str(&format!("    B{id} [label=\"{label}\"];\n"));
        }

        /* Create edges between basic blocks.  */
        for (id, block) in self.blocks.iter().enumerate() {
            for successor in &block.succs {
                dot.push_str(&format!("    B{id} -> B{};\n", successor.0));
            }
        }

        dot.push_str("}\n");

        dot
    }

    fn escape_dot(&self, s: &str) -> String {
        s.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "")
    }
}

struct BodyLayout<'a> {
    leaders: BTreeSet<usize>,
    labels: HashMap<&'a str, usize>,
}

impl BodyLayout<'_> {
    fn new() -> Self {
        Self {
            leaders: BTreeSet::new(),
            labels: HashMap::new(),
        }
    }
}

impl Default for BodyLayout<'_> {
    fn default() -> Self {
        Self::new()
    }
}

/// Iterate through the instructions and find the basic block leaders.
/// Besides returning the leaders, this fn also returns the labels.
fn analyze_body<I: CfgInstr>(instructions: &[I]) -> Result<BodyLayout<'_>, String> {
    let mut layout = BodyLayout::default();

    /* First instruction is always a leader.  */
    layout.leaders.insert(0);

    for (idx, instr) in instructions.iter().enumerate() {
        match instr.cfg_flow() {
            /* Labels start off a new basic block.  */
            CfgFlow::Label(name) => {
                if layout.labels.insert(name, idx).is_some() {
                    return Err(format!("Duplicate label: {name}"));
                }

                layout.leaders.insert(idx);
            }

            /* Both conditional and unconditional jumps, as well as
             * returns end the current basic block.
             *
             * The following instruction starts off a new basic block.  */
            CfgFlow::Jump(_) | CfgFlow::Branch(_) | CfgFlow::Return => {
                if idx + 1 < instructions.len() {
                    layout.leaders.insert(idx + 1);
                }
            }

            CfgFlow::Next => {}
        }
    }

    Ok(layout)
}

#[derive(Debug, Clone, Copy)]
pub enum CfgFlow<'a> {
    Label(&'a str),
    Jump(&'a str),
    Branch(&'a str),
    Return,
    Next,
}

pub trait CfgInstr {
    fn cfg_flow(&self) -> CfgFlow<'_>;
}

impl CfgInstr for IRInstr {
    fn cfg_flow(&self) -> CfgFlow<'_> {
        match self {
            IRInstr::Jz { target, .. } => CfgFlow::Branch(target),
            IRInstr::Jmp { target } => CfgFlow::Jump(target),
            IRInstr::Label(name) => CfgFlow::Label(name),
            IRInstr::Ret { .. } => CfgFlow::Return,
            _ => CfgFlow::Next,
        }
    }
}

impl CfgInstr for AsmInstr {
    fn cfg_flow(&self) -> CfgFlow<'_> {
        match self {
            AsmInstr::Label(name) => CfgFlow::Label(name),
            AsmInstr::Jmp { target } => CfgFlow::Jump(target),
            AsmInstr::Jmpcc { target, .. } => CfgFlow::Branch(target),
            AsmInstr::Ret => CfgFlow::Return,
            _ => CfgFlow::Next,
        }
    }
}
