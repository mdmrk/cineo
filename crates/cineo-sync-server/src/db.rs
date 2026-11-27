#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SqlValue {
    Null,
    Int(i64),
    Blob(Vec<u8>),
}

impl SqlValue {
    pub(crate) fn int(&self) -> Option<u64> {
        match self {
            Self::Int(value) => u64::try_from(*value).ok(),
            _ => None,
        }
    }

    pub(crate) fn blob(self) -> Option<Vec<u8>> {
        match self {
            Self::Blob(bytes) => Some(bytes),
            _ => None,
        }
    }

    pub(crate) fn from_u64(value: u64) -> Self {
        Self::Int(i64::try_from(value).unwrap_or(i64::MAX))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Statement {
    pub(crate) sql: String,
    pub(crate) params: Vec<SqlValue>,
}

impl Statement {
    pub(crate) fn new(sql: impl Into<String>, params: Vec<SqlValue>) -> Self {
        Self {
            sql: sql.into(),
            params,
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("storage failed: {0}")]
pub struct DbError(pub String);

pub(crate) trait Db {
    async fn rows(&self, statement: Statement) -> Result<Vec<Vec<SqlValue>>, DbError>;

    async fn batch(&self, statements: Vec<Statement>) -> Result<Vec<u64>, DbError>;
}
