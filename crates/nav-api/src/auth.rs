//! Authentification des clients par jeton d'accès.
//!
//! Le jeton est transmis dans l'en-tête `Authorization: Bearer …` (REST,
//! et WebSocket hors navigateur) ou dans le premier message d'une session
//! WebSocket. Il n'est jamais journalisé ni affiché par `Debug`.

use std::fmt;

use uuid::Uuid;

/// Longueur minimale d'un jeton accepté (caractères).
pub const MIN_TOKEN_LEN: usize = 32;

/// Longueur maximale d'un jeton accepté (caractères).
pub const MAX_TOKEN_LEN: usize = 256;

/// Jeton d'accès à l'API.
#[derive(Clone, PartialEq, Eq)]
pub struct ApiToken(String);

/// Jeton refusé (format invalide).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidToken;

impl fmt::Display for InvalidToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "jeton invalide : {MIN_TOKEN_LEN} à {MAX_TOKEN_LEN} caractères ASCII visibles"
        )
    }
}

impl std::error::Error for InvalidToken {}

impl ApiToken {
    /// Nouveau jeton aléatoire : 64 caractères hexadécimaux (244 bits
    /// aléatoires, générateur du système via `uuid` v4).
    #[must_use]
    pub fn generate() -> Self {
        Self(format!(
            "{}{}",
            Uuid::new_v4().simple(),
            Uuid::new_v4().simple()
        ))
    }

    /// Analyse un jeton (espaces de début et de fin ignorés).
    ///
    /// # Errors
    ///
    /// [`InvalidToken`] si sa longueur ou ses caractères sont invalides.
    pub fn parse(value: &str) -> Result<Self, InvalidToken> {
        let value = value.trim();
        let valid = (MIN_TOKEN_LEN..=MAX_TOKEN_LEN).contains(&value.len())
            && value.bytes().all(|b| b.is_ascii_graphic());
        if valid {
            Ok(Self(value.to_owned()))
        } else {
            Err(InvalidToken)
        }
    }

    /// Valeur du jeton (à écrire uniquement dans son fichier protégé).
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// Compare un jeton présenté, en temps constant pour une longueur
    /// donnée.
    #[must_use]
    pub fn matches(&self, presented: &str) -> bool {
        let (a, b) = (self.0.as_bytes(), presented.as_bytes());
        a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
    }
}

impl fmt::Debug for ApiToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ApiToken(***)")
    }
}

/// Extrait le jeton d'un en-tête `Authorization: Bearer …`.
#[must_use]
pub fn bearer(header: &str) -> Option<&str> {
    let (scheme, token) = header.trim().split_once(' ')?;
    scheme
        .eq_ignore_ascii_case("bearer")
        .then(|| token.trim())
        .filter(|t| !t.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_tokens_are_long_unique_and_valid() {
        let a = ApiToken::generate();
        let b = ApiToken::generate();
        assert_eq!(a.expose().len(), 64);
        assert!(a.expose().bytes().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
        assert_eq!(ApiToken::parse(a.expose()), Ok(a));
    }

    #[test]
    fn parsing_rejects_short_long_or_odd_tokens() {
        assert_eq!(ApiToken::parse("court"), Err(InvalidToken));
        assert_eq!(ApiToken::parse(&"x".repeat(257)), Err(InvalidToken));
        assert_eq!(
            ApiToken::parse(&format!("{} avec espace", "x".repeat(32))),
            Err(InvalidToken)
        );
        assert_eq!(ApiToken::parse(&"é".repeat(32)), Err(InvalidToken));
        assert!(ApiToken::parse(&format!("  {}\n", "a".repeat(40))).is_ok());
    }

    #[test]
    fn matching_is_exact() {
        let token = ApiToken::parse(&"a".repeat(40)).expect("jeton");
        assert!(token.matches(&"a".repeat(40)));
        assert!(!token.matches(&"a".repeat(39)));
        assert!(!token.matches(&format!("{}b", "a".repeat(39))));
        assert!(!token.matches(""));
    }

    #[test]
    fn debug_never_reveals_the_token() {
        let token = ApiToken::parse(&"s".repeat(40)).expect("jeton");
        assert_eq!(format!("{token:?}"), "ApiToken(***)");
    }

    #[test]
    fn bearer_header_parsing() {
        assert_eq!(bearer("Bearer abc"), Some("abc"));
        assert_eq!(bearer("bearer   abc "), Some("abc"));
        assert_eq!(bearer("Basic abc"), None);
        assert_eq!(bearer("Bearer"), None);
        assert_eq!(bearer("Bearer  "), None);
    }
}
