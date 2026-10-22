use crate::parser::ast::{BlockItem, Declaration, Param, Program, Statement, Type};

pub struct LoopLabeler {
    next_id: usize,
    active_loops: Vec<String>,
}

impl LoopLabeler {
    pub fn new() -> Self {
        Self {
            next_id: 0,
            active_loops: Vec::new(),
        }
    }

    fn get_next_label(&mut self) -> String {
        let label = format!("loop.{}", self.next_id);
        self.next_id += 1;
        label
    }

    fn current_loop(&self, keyword: &str) -> Result<String, LoopLabelError> {
        self.active_loops
            .last()
            .cloned()
            .ok_or_else(|| LoopLabelError {
                msg: format!("`{keyword} outside the loop"),
            })
    }
}

pub trait LoopLabel {
    type Output;

    fn loop_label(self, labeler: &mut LoopLabeler) -> Result<Self::Output, LoopLabelError>;
}

impl LoopLabel for Program {
    type Output = Program;

    fn loop_label(self, labeler: &mut LoopLabeler) -> Result<Self::Output, LoopLabelError> {
        Ok(Program {
            block_items: self
                .block_items
                .into_iter()
                .map(|block_item| block_item.loop_label(labeler))
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

impl LoopLabel for BlockItem {
    type Output = BlockItem;

    fn loop_label(self, labeler: &mut LoopLabeler) -> Result<Self::Output, LoopLabelError> {
        match self {
            BlockItem::Decl(decl) => Ok(BlockItem::Decl(decl.loop_label(labeler)?)),
            BlockItem::Stmt(stmt) => Ok(BlockItem::Stmt(stmt.loop_label(labeler)?)),
        }
    }
}

impl LoopLabel for Declaration {
    type Output = Declaration;

    fn loop_label(self, labeler: &mut LoopLabeler) -> Result<Self::Output, LoopLabelError> {
        match self {
            Declaration::Fn {
                name,
                ret,
                params,
                body,
            } => Ok(loop_label_fn(labeler, name, ret, params, body)?),
        }
    }
}

impl LoopLabel for Statement {
    type Output = Statement;

    fn loop_label(self, labeler: &mut LoopLabeler) -> Result<Self::Output, LoopLabelError> {
        match self {
            Statement::Print { value } => Ok(Statement::Print { value }),
            Statement::Let { name, init, ty } => Ok(Statement::Let { name, init, ty }),
            Statement::Ret { value, ret_ty } => Ok(Statement::Ret { value, ret_ty }),
            Statement::If {
                cond,
                then_block,
                else_block,
            } => Ok(Statement::If {
                cond,
                then_block: then_block.loop_label(labeler)?.into(),
                else_block: else_block
                    .map(|else_block| else_block.loop_label(labeler).map(Box::new))
                    .transpose()?,
            }),
            Statement::Block { body } => Ok(Self::Block {
                body: body
                    .into_iter()
                    .map(|stmt| stmt.loop_label(labeler))
                    .collect::<Result<Vec<_>, _>>()?,
            }),
            Statement::While { cond, body, .. } => {
                let label = labeler.get_next_label();

                /* enter the loop  */
                labeler.active_loops.push(label.clone());

                /* label the body  */
                let r = body.loop_label(labeler)?;

                /* exit the loop  */
                labeler.active_loops.pop();

                Ok(Statement::While {
                    cond,
                    body: r.into(),
                    label: Some(label),
                })
            }
            Statement::DoWhile { cond, body, .. } => {
                let label = labeler.get_next_label();

                /* enter the loop  */
                labeler.active_loops.push(label.clone());

                /* label the body  */
                let r = body.loop_label(labeler)?;

                /* exit the loop  */
                labeler.active_loops.pop();

                Ok(Statement::DoWhile {
                    cond,
                    body: r.into(),
                    label: Some(label),
                })
            }
            Self::For {
                init,
                cond,
                advancement,
                body,
                ..
            } => {
                let label = labeler.get_next_label();

                let init = init
                    .map(|stmt| stmt.loop_label(labeler).map(Box::new))
                    .transpose()?;

                labeler.active_loops.push(label.clone());

                let r = body.loop_label(labeler)?;

                labeler.active_loops.pop();

                Ok(Statement::For {
                    init,
                    cond,
                    advancement,
                    body: r.into(),
                    label: Some(label),
                })
            }
            Statement::Loop { body, .. } => {
                let label = labeler.get_next_label();

                labeler.active_loops.push(label.clone());

                let r = body.loop_label(labeler)?.into();

                labeler.active_loops.pop();

                Ok(Statement::Loop {
                    body: r,
                    label: Some(label),
                })
            }
            Statement::Break { .. } => {
                let label = labeler.current_loop("break")?;

                Ok(Statement::Break { label: Some(label) })
            }
            Statement::Continue { .. } => {
                let label = labeler.current_loop("continue")?;

                Ok(Statement::Continue { label: Some(label) })
            }
            _ => unreachable!(),
        }
    }
}

fn loop_label_fn(
    labeler: &mut LoopLabeler,
    name: String,
    ret: Option<Type>,
    params: Vec<Param>,
    body: Statement,
) -> Result<Declaration, LoopLabelError> {
    let old_loops = std::mem::take(&mut labeler.active_loops);

    let r = body.loop_label(labeler)?;

    labeler.active_loops = old_loops;

    Ok(Declaration::Fn {
        name,
        ret,
        params,
        body: r,
    })
}

#[derive(Debug)]
pub struct LoopLabelError {
    pub msg: String,
}

impl std::error::Error for LoopLabelError {}

impl std::fmt::Display for LoopLabelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "loop_labeler: {}", self.msg)
    }
}
