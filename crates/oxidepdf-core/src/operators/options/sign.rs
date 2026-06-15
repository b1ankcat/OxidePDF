use crate::{
    OxideError, SignatureAddOptions, SignatureDeleteFieldOptions, SignatureMode, SignatureOptions,
    TimestampAddOptions,
};
use serde::{Deserialize, Serialize};

/// PDF signing and signature verification operations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "PdfSignOptionsDef", into = "PdfSignOptionsDef")]
pub enum PdfSignOptions {
    /// Add a digital signature.
    Add(SignatureAddOptions),
    /// List PDF signatures without trust validation.
    List(SignatureOptions),
    /// Verify PDF signatures and certificate material.
    Verify(SignatureOptions),
    /// Delete a signature field.
    DeleteField(SignatureDeleteFieldOptions),
    /// Add or inspect a timestamp token.
    Timestamp(TimestampAddOptions),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct PdfSignOptionsDef {
    add: Option<SignatureAddOptions>,
    list: Option<SignatureOptions>,
    verify: Option<SignatureOptions>,
    delete_field: Option<SignatureDeleteFieldOptions>,
    timestamp: Option<TimestampAddOptions>,
}

impl TryFrom<PdfSignOptionsDef> for PdfSignOptions {
    type Error = OxideError;

    fn try_from(value: PdfSignOptionsDef) -> Result<Self, Self::Error> {
        let operation_count = [
            value.add.is_some(),
            value.list.is_some(),
            value.verify.is_some(),
            value.delete_field.is_some(),
            value.timestamp.is_some(),
        ]
        .into_iter()
        .filter(|present| *present)
        .count();

        if operation_count != 1 {
            return Err(OxideError::InvalidWorkflow {
                reason: "pdf_sign must contain exactly one operation".to_owned(),
            });
        }

        if let Some(options) = value.add {
            return Ok(Self::Add(options));
        }
        if let Some(mut options) = value.list {
            options.mode = SignatureMode::List;
            return Ok(Self::List(options));
        }
        if let Some(mut options) = value.verify {
            options.mode = SignatureMode::Verify;
            return Ok(Self::Verify(options));
        }
        if let Some(options) = value.delete_field {
            return Ok(Self::DeleteField(options));
        }
        if let Some(options) = value.timestamp {
            return Ok(Self::Timestamp(options));
        }

        unreachable!("operation count was already checked");
    }
}

impl From<PdfSignOptions> for PdfSignOptionsDef {
    fn from(value: PdfSignOptions) -> Self {
        match value {
            PdfSignOptions::Add(options) => Self {
                add: Some(options),
                ..Self::default()
            },
            PdfSignOptions::List(options) => Self {
                list: Some(options),
                ..Self::default()
            },
            PdfSignOptions::Verify(options) => Self {
                verify: Some(options),
                ..Self::default()
            },
            PdfSignOptions::DeleteField(options) => Self {
                delete_field: Some(options),
                ..Self::default()
            },
            PdfSignOptions::Timestamp(options) => Self {
                timestamp: Some(options),
                ..Self::default()
            },
        }
    }
}
