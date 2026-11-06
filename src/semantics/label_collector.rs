use crate::{QError, QErrorKind, tokenizer::token::Span};
use std::collections::HashSet;

use crate::parser::ast::{BlockItem, Declaration, Param, Program, Statement, Type};

pub struct LabelCollector {
    defined: HashSet<String>,
    referenced: Vec<String>,
}

impl LabelCollector {
    pub fn new() -> Self {
        Self {
            defined: HashSet::new(),
            referenced: Vec::new(),
        }
    }
}

impl Default for LabelCollector {
    fn default() -> Self {
        Self::new()
    }
}

pub trait LabelCollect {
    type Output;
    fn label_collect(self, collector: &mut LabelCollector) -> Result<Self::Output, QError>;
}

impl LabelCollect for Program {
    type Output = Program;

    fn label_collect(self, collector: &mut LabelCollector) -> Result<Self::Output, QError> {
        Ok(Program {
            block_items: self
                .block_items
                .into_iter()
                .map(|block_item| block_item.label_collect(collector))
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

impl LabelCollect for BlockItem {
    type Output = BlockItem;

    fn label_collect(self, collector: &mut LabelCollector) -> Result<Self::Output, QError> {
        match self {
            BlockItem::Decl(decl) => Ok(BlockItem::Decl(decl.label_collect(collector)?)),
            BlockItem::Stmt(stmt) => Ok(BlockItem::Stmt(stmt.label_collect(collector)?)),
        }
    }
}

impl LabelCollect for Declaration {
    type Output = Declaration;

    fn label_collect(self, collector: &mut LabelCollector) -> Result<Self::Output, QError> {
        match self {
            Declaration::Fn {
                name,
                ret,
                params,
                body,
            } => Ok(label_collect_fn(collector, name, ret, params, body)?),
            Declaration::Struct { name, members } => todo!(),
        }
    }
}

impl LabelCollect for Statement {
    type Output = Statement;

    fn label_collect(self, collector: &mut LabelCollector) -> Result<Self::Output, QError> {
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
                then_block: then_block.label_collect(collector)?.into(),
                else_block: else_block
                    .map(|else_block| else_block.label_collect(collector).map(Box::new))
                    .transpose()?,
            }),
            Statement::Block { body } => Ok(Statement::Block {
                body: body
                    .into_iter()
                    .map(|stmt| stmt.label_collect(collector))
                    .collect::<Result<Vec<_>, _>>()?,
            }),
            Statement::While { cond, body, label } => Ok(Statement::While {
                cond,
                body: body.label_collect(collector)?.into(),
                label,
            }),
            Statement::DoWhile { cond, body, label } => Ok(Statement::DoWhile {
                cond,
                body: body.label_collect(collector)?.into(),
                label,
            }),
            Statement::For {
                init,
                cond,
                advancement,
                body,
                label,
            } => Ok(Statement::For {
                init,
                cond,
                advancement,
                body: body.label_collect(collector)?.into(),
                label,
            }),
            Statement::Loop { body, label } => Ok(Statement::Loop {
                body: body.label_collect(collector)?.into(),
                label,
            }),
            Statement::Break { label } => Ok(Statement::Break { label }),
            Statement::Continue { label } => Ok(Statement::Continue { label }),
            Statement::Labeled { name, stmt } => {
                if !collector.defined.insert(name.clone()) {
                    return Err(QError {
                        code: 1,
                        kind: QErrorKind::LabelCollect,
                        msg: format!("duplicate label: '{name}'"),
                        span: Span { start: 0, end: 0 },
                    });
                }

                Ok(Statement::Labeled {
                    name,
                    stmt: stmt.label_collect(collector)?.into(),
                })
            }
            Statement::Goto { label } => {
                collector.referenced.push(label.clone());

                Ok(Statement::Goto { label })
            }
            Statement::Expr { expr } => Ok(Statement::Expr { expr }),
        }
    }
}

fn label_collect_fn(
    collector: &mut LabelCollector,
    name: String,
    ret: Option<Type>,
    params: Vec<Param>,
    body: Statement,
) -> Result<Declaration, QError> {
    collector.defined.clear();
    collector.referenced.clear();

    let body = body.label_collect(collector)?;

    /* each goto target must exist  */
    for target in &collector.referenced {
        if !collector.defined.contains(target) {
            return Err(QError {
                kind: QErrorKind::LabelCollect,
                code: 1,
                msg: format!("undefined label '{target}' in function '{name}'"),
                span: Span { start: 0, end: 0 },
            });
        }
    }

    Ok(Declaration::Fn {
        name,
        ret,
        params,
        body,
    })
}
