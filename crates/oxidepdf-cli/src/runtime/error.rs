use oxidepdf_core::OxideError;
use std::io;

#[derive(Debug)]
pub(crate) enum CliError {
    Arguments(clap::Error),
    Workflow(String),
    Input(io::Error),
    Io(io::Error),
    Core(OxideError),
}

impl CliError {
    pub(crate) fn exit_code(&self) -> i32 {
        match self {
            Self::Arguments(error) => error.exit_code(),
            Self::Workflow(_) => 2,
            Self::Input(_) => 3,
            Self::Core(OxideError::InvalidWorkflow { .. }) => 2,
            Self::Core(OxideError::InvalidInput { .. })
            | Self::Core(OxideError::UnsupportedPdfFeature { .. })
            | Self::Core(OxideError::ParsePdf)
            | Self::Core(OxideError::RenderPdf)
            | Self::Core(OxideError::ExtractText)
            | Self::Core(OxideError::SvgParse)
            | Self::Core(OxideError::ImageDecode) => 3,
            Self::Core(OxideError::EncryptedPdf) | Self::Core(OxideError::IncorrectPassword) => 4,
            Self::Core(OxideError::ResourceLimitExceeded { .. }) => 5,
            Self::Io(_)
            | Self::Core(OxideError::WritePdf)
            | Self::Core(OxideError::FontResolution)
            | Self::Core(OxideError::ArtifactStorage)
            | Self::Core(OxideError::Io)
            | Self::Core(OxideError::Internal) => 70,
        }
    }
}

impl std::fmt::Display for CliError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Arguments(error) => write!(formatter, "{error}"),
            Self::Workflow(error) => write!(formatter, "invalid workflow: {error}"),
            Self::Input(error) => write!(formatter, "input error: {}", sanitized_io_error(error)),
            Self::Io(error) => write!(formatter, "I/O error: {}", sanitized_io_error(error)),
            Self::Core(error) => write!(formatter, "{}: {error}", error.code()),
        }
    }
}

fn sanitized_io_error(error: &io::Error) -> &'static str {
    match error.kind() {
        io::ErrorKind::NotFound => "file not found",
        io::ErrorKind::PermissionDenied => "permission denied",
        io::ErrorKind::AlreadyExists => "file already exists",
        io::ErrorKind::InvalidInput => "invalid input",
        io::ErrorKind::InvalidData => "invalid data",
        io::ErrorKind::UnexpectedEof => "unexpected end of file",
        io::ErrorKind::WriteZero => "write failed",
        io::ErrorKind::Interrupted => "operation interrupted",
        io::ErrorKind::TimedOut => "operation timed out",
        _ => "operation failed",
    }
}
