//! Configuration d'une connexion Signal K.

use std::fmt;

use thiserror::Error;
use url::Url;

/// Longueur maximale d'un jeton.
pub const MAX_TOKEN_LEN: usize = 4_096;

/// Erreur de configuration.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ConfigError {
    /// URL illisible.
    #[error("URL invalide : {0}")]
    InvalidUrl(String),
    /// Schéma autre que http/https.
    #[error("schéma non pris en charge : {0} (http ou https attendu)")]
    UnsupportedScheme(String),
    /// Identifiants, requête ou fragment dans l'URL.
    #[error("l'URL ne doit contenir ni identifiants, ni paramètres, ni fragment")]
    UnexpectedUrlParts,
    /// Jeton vide, trop long ou contenant des caractères interdits.
    #[error("jeton invalide")]
    InvalidToken,
}

/// Jeton d'accès Signal K. Jamais affiché ni journalisé.
#[derive(Clone, PartialEq, Eq)]
pub struct Token(String);

impl Token {
    /// Valide un jeton : caractères ASCII visibles uniquement (pas
    /// d'espace ni de retour à la ligne : protection des en-têtes HTTP).
    ///
    /// # Errors
    ///
    /// [`ConfigError::InvalidToken`] si le jeton est invalide.
    pub fn new(value: &str) -> Result<Self, ConfigError> {
        let value = value.trim();
        if value.is_empty()
            || value.len() > MAX_TOKEN_LEN
            || !value.bytes().all(|b| b.is_ascii_graphic())
        {
            return Err(ConfigError::InvalidToken);
        }
        Ok(Self(value.to_owned()))
    }

    /// Valeur brute, pour l'en-tête `Authorization` uniquement.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Token(***)")
    }
}

/// Paramètres de connexion à un serveur Signal K.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalKConfig {
    base_url: Url,
    token: Option<Token>,
}

impl SignalKConfig {
    /// Crée une configuration à partir de l'adresse du serveur
    /// (ex. `http://raspberrypi.local:3000`) et d'un jeton optionnel.
    ///
    /// # Errors
    ///
    /// [`ConfigError`] si l'URL ou le jeton sont invalides.
    pub fn new(url: &str, token: Option<Token>) -> Result<Self, ConfigError> {
        let parsed = Url::parse(url.trim()).map_err(|e| ConfigError::InvalidUrl(e.to_string()))?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return Err(ConfigError::UnsupportedScheme(parsed.scheme().to_owned()));
        }
        if parsed.host_str().map_or(true, str::is_empty) {
            return Err(ConfigError::InvalidUrl("hôte absent".to_owned()));
        }
        if !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.query().is_some()
            || parsed.fragment().is_some()
        {
            return Err(ConfigError::UnexpectedUrlParts);
        }
        let mut base_url = parsed;
        base_url.set_path("/");
        Ok(Self { base_url, token })
    }

    /// Adresse racine du serveur.
    #[must_use]
    pub fn base_url(&self) -> &Url {
        &self.base_url
    }

    /// Jeton d'accès.
    #[must_use]
    pub fn token(&self) -> Option<&Token> {
        self.token.as_ref()
    }

    /// Adresse affichable (sans jeton).
    #[must_use]
    pub fn display_url(&self) -> String {
        self.base_url.as_str().trim_end_matches('/').to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_plain_server_addresses() {
        let c = SignalKConfig::new(" http://raspberrypi.local:3000/admin ", None).expect("ok");
        assert_eq!(c.base_url().as_str(), "http://raspberrypi.local:3000/");
        assert_eq!(c.display_url(), "http://raspberrypi.local:3000");
        assert!(SignalKConfig::new("https://10.10.10.1", None).is_ok());
    }

    #[test]
    fn rejects_unsafe_or_invalid_addresses() {
        assert!(matches!(
            SignalKConfig::new("ftp://host", None),
            Err(ConfigError::UnsupportedScheme(_))
        ));
        assert!(matches!(
            SignalKConfig::new("file:///etc/passwd", None),
            Err(ConfigError::UnsupportedScheme(_))
        ));
        assert!(matches!(
            SignalKConfig::new("pas une url", None),
            Err(ConfigError::InvalidUrl(_))
        ));
        assert_eq!(
            SignalKConfig::new("http://user:pw@host", None),
            Err(ConfigError::UnexpectedUrlParts)
        );
        assert_eq!(
            SignalKConfig::new("http://host/?token=x", None),
            Err(ConfigError::UnexpectedUrlParts)
        );
    }

    #[test]
    fn tokens_are_validated_and_never_printed() {
        let token = Token::new("  eyJhbGciOi.abc-_~  ").expect("ok");
        assert_eq!(token.expose(), "eyJhbGciOi.abc-_~");
        let c = SignalKConfig::new("http://h", Some(token)).expect("ok");
        let debug = format!("{c:?}");
        assert!(!debug.contains("eyJ"), "{debug}");
        assert!(debug.contains("***"));
        assert_eq!(Token::new(""), Err(ConfigError::InvalidToken));
        assert_eq!(
            Token::new("a\r\nX-Injected: 1"),
            Err(ConfigError::InvalidToken)
        );
        assert_eq!(Token::new("a b"), Err(ConfigError::InvalidToken));
        assert_eq!(
            Token::new(&"x".repeat(MAX_TOKEN_LEN + 1)),
            Err(ConfigError::InvalidToken)
        );
    }
}
