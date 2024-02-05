use std::sync::Arc;

use bytes::Bytes;
use postgres_types::FromSqlOwned;

use crate::{
    api::Type,
    error::{PgWireError, PgWireResult},
    messages::{data::FORMAT_CODE_BINARY, extendedquery::Bind},
};

use super::{results::FieldFormat, stmt::StoredStatement, DEFAULT_NAME};

/// Represent a prepared sql statement and its parameters bound by a `Bind`
/// request.
#[non_exhaustive]
#[derive(Debug, Default, Clone)]
pub struct Portal<S> {
    pub name: String,
    pub statement: Arc<StoredStatement<S>>,
    pub parameter_format: Format,
    pub parameters: Vec<Option<Bytes>>,
    pub result_column_format: Format,
}

#[derive(Debug, Clone, Default)]
pub enum Format {
    #[default]
    UnifiedText,
    UnifiedBinary,
    Individual(Vec<i16>),
}

impl From<i16> for Format {
    fn from(v: i16) -> Format {
        if v == FORMAT_CODE_BINARY {
            Format::UnifiedBinary
        } else {
            Format::UnifiedText
        }
    }
}

impl Format {
    /// Get format code for given index
    pub fn format_for(&self, idx: usize) -> FieldFormat {
        match self {
            Format::UnifiedText => FieldFormat::Text,
            Format::UnifiedBinary => FieldFormat::Binary,
            Format::Individual(ref fv) => FieldFormat::from(fv[idx]),
        }
    }

    /// Test if `idx` field is text format
    pub fn is_text(&self, idx: usize) -> bool {
        self.format_for(idx) == FieldFormat::Text
    }

    /// Test if `idx` field is binary format
    pub fn is_binary(&self, idx: usize) -> bool {
        self.format_for(idx) == FieldFormat::Binary
    }

    fn from_codes(codes: &[i16]) -> Self {
        if codes.is_empty() {
            Format::UnifiedText
        } else if codes.len() == 1 {
            Format::from(codes[0])
        } else {
            Format::Individual(codes.to_vec())
        }
    }
}

impl<S: Clone> Portal<S> {
    /// Try to create portal from bind command and current client state
    pub fn try_new(bind: &Bind, statement: Arc<StoredStatement<S>>) -> PgWireResult<Self> {
        let portal_name = bind
            .portal_name
            .clone()
            .unwrap_or_else(|| DEFAULT_NAME.to_owned());

        // param format
        let param_format = Format::from_codes(&bind.parameter_format_codes);

        // format
        let result_format = Format::from_codes(&bind.result_column_format_codes);

        Ok(Portal {
            name: portal_name,
            statement,
            parameter_format: param_format,
            parameters: bind.parameters.clone(),
            result_column_format: result_format,
        })
    }

    /// Get number of parameters
    pub fn parameter_len(&self) -> usize {
        self.parameters.len()
    }

    /// Attempt to get parameter at given index as type `T`.
    ///
    pub fn parameter<T>(&self, idx: usize, pg_type: &Type) -> PgWireResult<Option<T>>
    where
        T: FromSqlOwned,
    {
        if !T::accepts(pg_type) {
            return Err(PgWireError::InvalidRustTypeForParameter(
                pg_type.name().to_owned(),
            ));
        }

        let param = self
            .parameters
            .get(idx)
            .ok_or_else(|| PgWireError::ParameterIndexOutOfBound(idx))?;

        let _format = self.parameter_format.format_for(idx);

        if let Some(ref param) = param {
            // TODO: from_sql only works with binary format
            // here we need to check format code first and seek to support text
            T::from_sql(pg_type, param)
                .map(|v| Some(v))
                .map_err(PgWireError::FailedToParseParameter)
        } else {
            // Null
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use postgres_types::FromSql;

    use super::*;

    #[test]
    fn test_from_sql() {
        assert_eq!(
            "helloworld",
            String::from_sql(&Type::UNKNOWN, "helloworld".as_bytes()).unwrap()
        )
    }
}
