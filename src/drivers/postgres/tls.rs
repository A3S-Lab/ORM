use std::fmt;
use std::sync::Arc;

use rustls::pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer};
use rustls::sign::{CertifiedKey, SingleCertAndKey};
use rustls::{ClientConfig, RootCertStore};
use tokio_postgres::config::{Host, SslMode};
use tokio_postgres::Config;
use tokio_postgres_rustls::MakeRustlsConnect;
use zeroize::{Zeroize, Zeroizing};

/// Certificate material for verified PostgreSQL TLS connections.
///
/// The private key is zeroized when the final clone is dropped, and `Debug`
/// never prints PEM contents.
#[derive(Clone)]
pub struct PostgresTlsOptions {
    root_certificates_pem: Arc<[u8]>,
    client_identity: Option<PostgresClientIdentity>,
}

impl PostgresTlsOptions {
    pub fn new(root_certificates_pem: impl Into<Vec<u8>>) -> Self {
        Self {
            root_certificates_pem: Arc::from(root_certificates_pem.into()),
            client_identity: None,
        }
    }

    pub fn with_client_identity(
        mut self,
        certificate_chain_pem: impl Into<Vec<u8>>,
        private_key_pem: impl Into<Vec<u8>>,
    ) -> Self {
        self.client_identity = Some(PostgresClientIdentity {
            certificate_chain_pem: Arc::from(certificate_chain_pem.into()),
            private_key_pem: Arc::new(Zeroizing::new(private_key_pem.into())),
        });
        self
    }

    pub const fn has_client_identity(&self) -> bool {
        self.client_identity.is_some()
    }

    pub fn validate(&self) -> Result<(), PostgresTlsError> {
        let _ = self.connector()?;
        Ok(())
    }

    pub(crate) fn validate_connection_config(
        &self,
        config: &Config,
    ) -> Result<(), PostgresTlsError> {
        if config.get_ssl_mode() != SslMode::Require {
            return Err(PostgresTlsError::TlsNotRequired);
        }
        if config
            .get_hosts()
            .iter()
            .any(|host| matches!(host, Host::Unix(_)))
        {
            return Err(PostgresTlsError::UnixSocket);
        }
        self.validate()
    }

    pub(crate) fn connector(&self) -> Result<MakeRustlsConnect, PostgresTlsError> {
        let roots = parse_root_certificates(&self.root_certificates_pem)?;
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let builder = ClientConfig::builder_with_provider(Arc::clone(&provider))
            .with_safe_default_protocol_versions()
            .map_err(|_| PostgresTlsError::UnsupportedProtocolVersions)?
            .with_root_certificates(roots);
        let config = match &self.client_identity {
            Some(identity) => {
                let certificates = parse_client_certificates(&identity.certificate_chain_pem)?;
                let mut private_key = PrivateKeyDer::from_pem_slice(
                    identity.private_key_pem.as_slice(),
                )
                .map_err(|error| match error {
                    rustls::pki_types::pem::Error::NoItemsFound => {
                        PostgresTlsError::MissingPrivateKey
                    }
                    _ => PostgresTlsError::InvalidPrivateKey,
                })?;
                let signing_key = rustls::crypto::ring::sign::any_supported_type(&private_key);
                private_key.zeroize();
                let certified_key = CertifiedKey::new(
                    certificates,
                    signing_key.map_err(|_| PostgresTlsError::InvalidClientIdentity)?,
                );
                certified_key
                    .keys_match()
                    .map_err(|_| PostgresTlsError::InvalidClientIdentity)?;
                builder.with_client_cert_resolver(Arc::new(SingleCertAndKey::from(certified_key)))
            }
            None => builder.with_no_client_auth(),
        };
        Ok(MakeRustlsConnect::new(config))
    }
}

impl fmt::Debug for PostgresTlsOptions {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PostgresTlsOptions")
            .field("root_certificates_pem", &"[REDACTED]")
            .field("client_identity", &self.client_identity)
            .finish()
    }
}

#[derive(Clone)]
struct PostgresClientIdentity {
    certificate_chain_pem: Arc<[u8]>,
    private_key_pem: Arc<Zeroizing<Vec<u8>>>,
}

impl fmt::Debug for PostgresClientIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PostgresClientIdentity")
            .field("certificate_chain_pem", &"[REDACTED]")
            .field("private_key_pem", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum PostgresTlsError {
    #[error("PostgreSQL TLS has no supported protocol versions")]
    UnsupportedProtocolVersions,
    #[error("PostgreSQL TLS requires at least one valid root certificate")]
    EmptyRootCertificates,
    #[error("PostgreSQL TLS root certificate PEM is invalid")]
    InvalidRootCertificate,
    #[error("PostgreSQL TLS requires sslmode=require")]
    TlsNotRequired,
    #[error("PostgreSQL TLS does not support Unix-socket hosts")]
    UnixSocket,
    #[error("PostgreSQL client certificate PEM is empty")]
    EmptyClientCertificates,
    #[error("PostgreSQL client certificate PEM is invalid")]
    InvalidClientCertificate,
    #[error("PostgreSQL client private key PEM contains no supported private key")]
    MissingPrivateKey,
    #[error("PostgreSQL client private key PEM is invalid")]
    InvalidPrivateKey,
    #[error("PostgreSQL client certificate and private key do not form a valid identity")]
    InvalidClientIdentity,
}

fn parse_root_certificates(pem: &[u8]) -> Result<RootCertStore, PostgresTlsError> {
    let certificates = CertificateDer::pem_slice_iter(pem)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| PostgresTlsError::InvalidRootCertificate)?;
    if certificates.is_empty() {
        return Err(PostgresTlsError::EmptyRootCertificates);
    }
    let mut roots = RootCertStore::empty();
    for certificate in certificates {
        roots
            .add(certificate)
            .map_err(|_| PostgresTlsError::InvalidRootCertificate)?;
    }
    Ok(roots)
}

fn parse_client_certificates(pem: &[u8]) -> Result<Vec<CertificateDer<'static>>, PostgresTlsError> {
    let certificates = CertificateDer::pem_slice_iter(pem)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| PostgresTlsError::InvalidClientCertificate)?;
    if certificates.is_empty() {
        return Err(PostgresTlsError::EmptyClientCertificates);
    }
    Ok(certificates)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Public, intentionally insecure fixture material. Keeping the base64 body
    // separate from the PEM label prevents it from being mistaken for a
    // deployable key file.
    const TEST_CERTIFICATE_BODY: &str = concat!(
        "MIIBlTCCATugAwIBAgIJANJAIwzq/ruBMAoGCCqGSM49BAMCMC4xLDAqBgNVBAMM",
        "I0EzUyBPUk0gSW5zZWN1cmUgVW5pdCBUZXN0IElkZW50aXR5MB4XDTI2MDcxOTA3",
        "MzcxOVoXDTM2MDcxNjA3MzcxOVowLjEsMCoGA1UEAwwjQTNTIE9STSBJbnNlY3Vy",
        "ZSBVbml0IFRlc3QgSWRlbnRpdHkwWTATBgcqhkjOPQIBBggqhkjOPQMBBwNCAATS",
        "5fyxkKzAw8H8YX7IvJcSHD9KZmN3wjiQbHAHwNjoxT4IzKXV7yp3oiXbDhw42ump",
        "bLnJk7kTR0n/fp/stuMMo0IwQDAPBgNVHRMBAf8EBTADAQH/MA4GA1UdDwEB/wQE",
        "AwIChDAdBgNVHSUEFjAUBggrBgEFBQcDAgYIKwYBBQUHAwEwCgYIKoZIzj0EAwID",
        "SAAwRQIhAMg99tKtlZPYR7q63zP7LReQ4PmfkXfSLC2cRLK6T0lVAiAOTCaLluRL",
        "bsZW5TRAQ6GMlrmV1wx3tz2dbTbgh7E/fQ==",
    );
    const TEST_PRIVATE_KEY_BODY: &str = concat!(
        "MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgRkUYlxC46S9fPuxu",
        "nr4SpMyuOgaOiMWL5XjMmwwSFC2hRANCAATS5fyxkKzAw8H8YX7IvJcSHD9KZmN3",
        "wjiQbHAHwNjoxT4IzKXV7yp3oiXbDhw42umpbLnJk7kTR0n/fp/stuMM",
    );

    fn test_pem(label: &str, body: &str) -> Vec<u8> {
        format!("-----BEGIN {label}-----\n{body}\n-----END {label}-----\n").into_bytes()
    }

    fn test_identity() -> (Vec<u8>, Vec<u8>) {
        (
            test_pem("CERTIFICATE", TEST_CERTIFICATE_BODY),
            test_pem("PRIVATE KEY", TEST_PRIVATE_KEY_BODY),
        )
    }

    #[test]
    fn debug_output_redacts_all_pem_material() {
        let options = PostgresTlsOptions::new(b"root-marker".to_vec())
            .with_client_identity(b"certificate-marker".to_vec(), b"key-marker".to_vec());
        let debug = format!("{options:?}");
        assert!(!debug.contains("root-marker"));
        assert!(!debug.contains("certificate-marker"));
        assert!(!debug.contains("key-marker"));
        assert!(debug.contains("[REDACTED]"));
    }

    #[test]
    fn invalid_and_missing_pem_are_rejected_without_echoing_input() {
        let error = PostgresTlsOptions::new(Vec::new()).validate().unwrap_err();
        assert_eq!(error, PostgresTlsError::EmptyRootCertificates);

        let error = PostgresTlsOptions::new(b"private-password-marker".to_vec())
            .validate()
            .unwrap_err();
        let rendered = format!("{error:?} {error}");
        assert_eq!(error, PostgresTlsError::EmptyRootCertificates);
        assert!(!rendered.contains("private-password-marker"));
    }

    #[test]
    fn tls_requires_explicit_sslmode_and_tcp_hosts() {
        let options = PostgresTlsOptions::new(Vec::new());
        let config = "postgres://localhost/database".parse::<Config>().unwrap();
        assert_eq!(
            options.validate_connection_config(&config),
            Err(PostgresTlsError::TlsNotRequired)
        );
        let config = "host=/tmp sslmode=require dbname=database"
            .parse::<Config>()
            .unwrap();
        assert_eq!(
            options.validate_connection_config(&config),
            Err(PostgresTlsError::UnixSocket)
        );
    }

    #[test]
    fn well_formed_root_and_client_identity_build_a_redacted_connector() {
        let (certificate, private_key) = test_identity();
        let options = PostgresTlsOptions::new(certificate.clone())
            .with_client_identity(certificate, private_key);
        assert!(options.has_client_identity());
        options.validate().unwrap();
        options.clone().validate().unwrap();
        let debug = format!("{options:?}");
        assert!(!debug.contains("PRIVATE KEY"));
        assert!(!debug.contains("CERTIFICATE"));
    }

    #[test]
    fn malformed_identity_material_is_rejected_by_category() {
        let (certificate, private_key) = test_identity();
        let malformed = b"-----BEGIN PRIVATE KEY-----\n!!!\n-----END PRIVATE KEY-----\n";
        assert_eq!(
            PostgresTlsOptions::new(certificate.clone())
                .with_client_identity(certificate.clone(), Vec::new())
                .validate(),
            Err(PostgresTlsError::MissingPrivateKey)
        );
        assert_eq!(
            PostgresTlsOptions::new(certificate.clone())
                .with_client_identity(certificate.clone(), malformed.to_vec())
                .validate(),
            Err(PostgresTlsError::InvalidPrivateKey)
        );
        assert_eq!(
            PostgresTlsOptions::new(certificate)
                .with_client_identity(Vec::new(), private_key)
                .validate(),
            Err(PostgresTlsError::EmptyClientCertificates)
        );
    }
}
