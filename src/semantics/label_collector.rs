use crate::parser::ast::{BlockItem, Declaration, Param, Program, Statement, Type};

pub struct LabelCollector {}

impl LabelCollector {
    pub fn new() -> Self {
        Self {}
    }
}

pub trait LabelCollect {
    type Output;

    fn label_collect(
        self,
        collector: &mut LabelCollector,
    ) -> Result<Self::Output, LabelCollectError>;
}

impl LabelCollect for Program {
    type Output = Program;

    fn label_collect(
        self,
        collector: &mut LabelCollector,
    ) -> Result<Self::Output, LabelCollectError> {
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

    fn label_collect(
        self,
        collector: &mut LabelCollector,
    ) -> Result<Self::Output, LabelCollectError> {
        match self {
            BlockItem::Decl(decl) => Ok(BlockItem::Decl(decl.label_collect(collector)?)),
            BlockItem::Stmt(stmt) => Ok(BlockItem::Stmt(stmt.label_collect(collector)?)),
        }
    }
}

impl LabelCollect for Declaration {
    type Output = Declaration;

    fn label_collect(
        self,
        collector: &mut LabelCollector,
    ) -> Result<Self::Output, LabelCollectError> {
        match self {
            Declaration::Fn {
                name,
                ret,
                params,
                body,
            } => Ok(label_collect_fn(collector, name, ret, params, body)?),
        }
    }
}

impl LabelCollect for Statement {
    type Output = Statement;

    fn label_collect(
        self,
        collector: &mut LabelCollector,
    ) -> Result<Self::Output, LabelCollectError> {
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
            _ => unimplemented!(),
        }
    }
}

fn label_collect_fn(
    collector: &mut LabelCollector,
    name: String,
    ret: Option<Type>,
    params: Vec<Param>,
    body: Statement,
) -> Result<Declaration, LabelCollectError> {
    Ok(Declaration::Fn {
        name,
        ret,
        params,
        body,
    })
}

#[derive(Debug)]
pub struct LabelCollectError {
    pub msg: String,
}

impl std::error::Error for LabelCollectError {}

impl std::fmt::Display for LabelCollectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "label_collector: {}", self.msg)
    }
}
