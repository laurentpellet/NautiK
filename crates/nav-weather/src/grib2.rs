//! Décodage GRIB2 (WMO FM 92) : messages, sections et champs.
//!
//! Pris en charge :
//! - grille 3.0 (latitude/longitude régulière), balayages usuels ;
//! - produits 4.0 et 4.1 (valeur instantanée, déterministe ou ensemble) ;
//! - compactages 5.0 (simple), 5.2 (complexe) et 5.3 (complexe avec
//!   différences spatiales d'ordre 1 ou 2) ;
//! - bitmap (section 6), y compris « bitmap précédent ».
//!
//! Un champ d'un type non pris en charge (autre grille, JPEG2000…) est
//! ignoré et signalé ; une structure corrompue est une erreur. Toutes
//! les longueurs sont vérifiées : aucune lecture hors des données.

use chrono::{DateTime, Duration, NaiveDate, Utc};
use thiserror::Error;

use crate::bits::Bits;

/// Taille maximale d'un fichier GRIB accepté (octets).
pub const MAX_FILE_BYTES: usize = 200 * 1024 * 1024;
/// Nombre maximal de points d'une grille (GFS 0,25° global : 1 038 240).
pub const MAX_GRID_POINTS: usize = 4_000_000;

/// Erreur de décodage.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum GribError {
    /// Aucun message GRIB dans les données.
    #[error("aucun message GRIB")]
    NoMessage,
    /// Données plus courtes qu'annoncé.
    #[error("message GRIB tronqué ({0})")]
    Truncated(&'static str),
    /// Structure invalide.
    #[error("GRIB invalide : {0}")]
    Invalid(String),
    /// Fichier trop volumineux.
    #[error("fichier GRIB trop volumineux ({0} octets)")]
    TooLarge(usize),
}

/// Grille latitude/longitude régulière (degrés, WGS84 approché).
#[derive(Debug, Clone, PartialEq)]
pub struct LatLonGrid {
    /// Nombre de points par ligne.
    pub ni: usize,
    /// Nombre de lignes.
    pub nj: usize,
    /// Premier point.
    pub lat1: f64,
    /// Premier point.
    pub lon1: f64,
    /// Pas en longitude (positif).
    pub di: f64,
    /// Pas en latitude (positif).
    pub dj: f64,
    /// Longitudes croissantes le long d'une ligne.
    pub i_positive: bool,
    /// Latitudes croissantes d'une ligne à l'autre.
    pub j_positive: bool,
}

impl LatLonGrid {
    /// Nombre de points.
    #[must_use]
    pub fn len(&self) -> usize {
        self.ni * self.nj
    }

    /// Grille vide ?
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Coordonnées `(latitude, longitude ∈ [-180, 180[)` du point d'indice
    /// `(i, j)` (ordre de stockage).
    #[must_use]
    pub fn point(&self, i: usize, j: usize) -> (f64, f64) {
        let di = if self.i_positive { self.di } else { -self.di };
        let dj = if self.j_positive { self.dj } else { -self.dj };
        let lat = self.lat1 + j as f64 * dj;
        let lon = (self.lon1 + i as f64 * di + 180.0).rem_euclid(360.0) - 180.0;
        (lat, lon)
    }
}

/// Champ décodé.
#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    /// Discipline (0 : météorologie).
    pub discipline: u8,
    /// Catégorie de paramètre.
    pub category: u8,
    /// Numéro de paramètre.
    pub number: u8,
    /// Type de la surface (103 : hauteur au-dessus du sol).
    pub surface_type: u8,
    /// Valeur de la surface (m pour 103).
    pub surface_value: f64,
    /// Heure de référence (analyse).
    pub reference_time: DateTime<Utc>,
    /// Heure de validité.
    pub valid_time: DateTime<Utc>,
    /// Grille.
    pub grid: LatLonGrid,
    /// Valeurs (ordre de stockage) ; `None` : valeur absente.
    pub values: Vec<Option<f32>>,
}

/// Résultat du décodage d'un fichier.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Decoded {
    /// Champs décodés.
    pub fields: Vec<Field>,
    /// Champs ignorés (type non pris en charge), avec la raison.
    pub skipped: Vec<String>,
}

fn u16_at(s: &[u8], i: usize) -> Result<u16, GribError> {
    s.get(i..i + 2)
        .map(|b| u16::from_be_bytes([b[0], b[1]]))
        .ok_or(GribError::Truncated("section"))
}

fn u32_at(s: &[u8], i: usize) -> Result<u32, GribError> {
    s.get(i..i + 4)
        .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
        .ok_or(GribError::Truncated("section"))
}

fn u8_at(s: &[u8], i: usize) -> Result<u8, GribError> {
    s.get(i).copied().ok_or(GribError::Truncated("section"))
}

/// Entier 32 bits « signe + valeur absolue ».
fn i32_sm(raw: u32) -> i64 {
    let magnitude = i64::from(raw & 0x7FFF_FFFF);
    if raw & 0x8000_0000 != 0 {
        -magnitude
    } else {
        magnitude
    }
}

fn i16_sm(raw: u16) -> i32 {
    let magnitude = i32::from(raw & 0x7FFF);
    if raw & 0x8000 != 0 {
        -magnitude
    } else {
        magnitude
    }
}

fn i8_sm(raw: u8) -> i32 {
    let magnitude = i32::from(raw & 0x7F);
    if raw & 0x80 != 0 {
        -magnitude
    } else {
        magnitude
    }
}

fn parse_reference_time(s: &[u8]) -> Result<DateTime<Utc>, GribError> {
    if s.len() < 21 {
        return Err(GribError::Truncated("section 1"));
    }
    let year = i32::from(u16_at(s, 12)?);
    let (month, day, hour, minute, second) = (s[14], s[15], s[16], s[17], s[18]);
    NaiveDate::from_ymd_opt(year, u32::from(month), u32::from(day))
        .and_then(|d| d.and_hms_opt(u32::from(hour), u32::from(minute), u32::from(second)))
        .map(|t| t.and_utc())
        .ok_or_else(|| GribError::Invalid(format!("date de référence {year}-{month}-{day}")))
}

/// Grille ; `Err(raison)` si elle n'est pas prise en charge.
fn parse_grid(s: &[u8]) -> Result<Result<LatLonGrid, String>, GribError> {
    if s.len() < 14 {
        return Err(GribError::Truncated("section 3"));
    }
    let template = u16_at(s, 12)?;
    if s[5] != 0 || s[10] != 0 {
        return Ok(Err("grille non régulière".to_owned()));
    }
    if template != 0 {
        return Ok(Err(format!("grille 3.{template}")));
    }
    if s.len() < 72 {
        return Err(GribError::Truncated("section 3"));
    }
    let npoints = u32_at(s, 6)? as usize;
    let ni = u32_at(s, 30)? as usize;
    let nj = u32_at(s, 34)? as usize;
    if ni == 0 || nj == 0 || ni.saturating_mul(nj) != npoints {
        return Err(GribError::Invalid(format!(
            "grille {ni}×{nj} pour {npoints} points"
        )));
    }
    if npoints > MAX_GRID_POINTS {
        return Ok(Err(format!("grille de {npoints} points")));
    }
    let basic = u32_at(s, 38)?;
    let subdivisions = u32_at(s, 42)?;
    let unit = if basic == 0 || basic == u32::MAX {
        1e-6
    } else if subdivisions == 0 || subdivisions == u32::MAX {
        return Err(GribError::Invalid("subdivisions d'angle nulles".to_owned()));
    } else {
        f64::from(basic) / f64::from(subdivisions)
    };
    let lat1 = i32_sm(u32_at(s, 46)?) as f64 * unit;
    let lon1 = i32_sm(u32_at(s, 50)?) as f64 * unit;
    let lat2 = i32_sm(u32_at(s, 55)?) as f64 * unit;
    let lon2 = i32_sm(u32_at(s, 59)?) as f64 * unit;
    let scan = s[71];
    if scan & 0x30 != 0 {
        return Ok(Err(format!("balayage 0x{scan:02x}")));
    }
    let step = |raw: u32, a: f64, b: f64, n: usize| -> f64 {
        if raw == u32::MAX {
            if n > 1 {
                (b - a).abs() / (n - 1) as f64
            } else {
                0.0
            }
        } else {
            f64::from(raw) * unit
        }
    };
    let di = step(u32_at(s, 63)?, lon1, lon2, ni);
    let dj = step(u32_at(s, 67)?, lat1, lat2, nj);
    if !(-90.0..=90.0).contains(&lat1) || !(-90.0..=90.0).contains(&lat2) {
        return Err(GribError::Invalid(format!("latitudes {lat1} / {lat2}")));
    }
    if (ni > 1 && !(di > 0.0 && di <= 360.0)) || (nj > 1 && !(dj > 0.0 && dj <= 180.0)) {
        return Err(GribError::Invalid(format!("pas de grille {di} / {dj}")));
    }
    Ok(Ok(LatLonGrid {
        ni,
        nj,
        lat1,
        lon1,
        di,
        dj,
        i_positive: scan & 0x80 == 0,
        j_positive: scan & 0x40 != 0,
    }))
}

#[derive(Debug, Clone)]
struct Product {
    category: u8,
    number: u8,
    forecast: Duration,
    surface_type: u8,
    surface_value: f64,
}

fn parse_product(s: &[u8]) -> Result<Result<Product, String>, GribError> {
    if s.len() < 9 {
        return Err(GribError::Truncated("section 4"));
    }
    let template = u16_at(s, 7)?;
    if template != 0 && template != 1 {
        return Ok(Err(format!("produit 4.{template}")));
    }
    if s.len() < 34 {
        return Err(GribError::Truncated("section 4"));
    }
    let unit_s: i64 = match s[17] {
        0 => 60,
        1 => 3600,
        2 => 86_400,
        10 => 3 * 3600,
        11 => 6 * 3600,
        12 => 12 * 3600,
        13 => 1,
        other => return Ok(Err(format!("unité de temps {other}"))),
    };
    let forecast = Duration::seconds(i64::from(u32_at(s, 18)?) * unit_s);
    let scale = i8_sm(s[23]);
    let scaled = f64::from(u32_at(s, 24)?);
    Ok(Ok(Product {
        category: s[9],
        number: s[10],
        forecast,
        surface_type: s[22],
        surface_value: scaled / 10f64.powi(scale),
    }))
}

#[derive(Debug, Clone)]
struct Complex {
    groups: usize,
    missing: u8,
    width_ref: u32,
    width_bits: u32,
    length_ref: u32,
    length_inc: u32,
    last_length: u32,
    length_bits: u32,
    /// Ordre de différenciation spatiale et taille des descripteurs.
    differencing: Option<(u8, u32)>,
}

#[derive(Debug, Clone)]
struct Packing {
    count: usize,
    reference: f64,
    binary_scale: i32,
    decimal_scale: i32,
    bits: u32,
    complex: Option<Complex>,
}

fn parse_packing(s: &[u8]) -> Result<Result<Packing, String>, GribError> {
    if s.len() < 21 {
        return Err(GribError::Truncated("section 5"));
    }
    let template = u16_at(s, 9)?;
    if !matches!(template, 0 | 2 | 3) {
        return Ok(Err(format!("compactage 5.{template}")));
    }
    let reference = f64::from(f32::from_bits(u32_at(s, 11)?));
    if !reference.is_finite() {
        return Err(GribError::Invalid(
            "valeur de référence non finie".to_owned(),
        ));
    }
    let complex = if template == 0 {
        None
    } else {
        if s.len() < 47 || (template == 3 && s.len() < 49) {
            return Err(GribError::Truncated("section 5"));
        }
        let differencing = if template == 3 {
            let order = s[47];
            let octets = u32::from(s[48]);
            if !(1..=2).contains(&order) || !(1..=4).contains(&octets) {
                return Ok(Err(format!("différences spatiales {order}/{octets}")));
            }
            Some((order, octets * 8))
        } else {
            None
        };
        Some(Complex {
            groups: u32_at(s, 31)? as usize,
            missing: s[22],
            width_ref: u32::from(s[35]),
            width_bits: u32::from(s[36]),
            length_ref: u32_at(s, 37)?,
            length_inc: u32::from(s[41]),
            last_length: u32_at(s, 42)?,
            length_bits: u32::from(s[46]),
            differencing,
        })
    };
    if complex.as_ref().is_some_and(|c| c.missing > 2) {
        return Ok(Err("gestion des valeurs absentes inconnue".to_owned()));
    }
    Ok(Ok(Packing {
        count: u32_at(s, 5)? as usize,
        reference,
        binary_scale: i16_sm(u16_at(s, 15)?),
        decimal_scale: i16_sm(u16_at(s, 17)?),
        bits: u32::from(s[19]),
        complex,
    }))
}

impl Packing {
    fn scale(&self, x: i64) -> f32 {
        let value = (self.reference + x as f64 * 2f64.powi(self.binary_scale))
            * 10f64.powi(-self.decimal_scale);
        value as f32
    }

    /// Valeurs compactées (ordre des points présents).
    fn unpack(&self, data: &[u8]) -> Result<Vec<Option<f32>>, GribError> {
        if self.count > MAX_GRID_POINTS {
            return Err(GribError::Invalid(format!("{} valeurs", self.count)));
        }
        if self.bits > 32 {
            return Err(GribError::Invalid(format!("{} bits par valeur", self.bits)));
        }
        match &self.complex {
            None => self.unpack_simple(data),
            Some(c) => self.unpack_complex(c, data),
        }
    }

    fn unpack_simple(&self, data: &[u8]) -> Result<Vec<Option<f32>>, GribError> {
        if self.bits == 0 {
            return Ok(vec![Some(self.scale(0)); self.count]);
        }
        let needed = (self.count * self.bits as usize).div_ceil(8);
        if data.len() < needed {
            return Err(GribError::Truncated("section 7"));
        }
        let mut bits = Bits::new(data);
        (0..self.count)
            .map(|_| Ok(Some(self.scale(i64::from(bits.read(self.bits)?)))))
            .collect()
    }

    fn unpack_complex(&self, c: &Complex, data: &[u8]) -> Result<Vec<Option<f32>>, GribError> {
        if c.groups == 0 {
            return if self.count == 0 {
                Ok(Vec::new())
            } else {
                Err(GribError::Invalid("aucun groupe".to_owned()))
            };
        }
        if c.groups > self.count {
            return Err(GribError::Invalid(format!(
                "{} groupes pour {} valeurs",
                c.groups, self.count
            )));
        }
        let mut bits = Bits::new(data);
        let mut first = [0i64; 2];
        let mut minimum = 0i64;
        if let Some((order, width)) = c.differencing {
            first[0] = bits.read_signed(width)?;
            if order == 2 {
                first[1] = bits.read_signed(width)?;
            }
            minimum = bits.read_signed(width)?;
        }
        bits.align();
        let refs = (0..c.groups)
            .map(|_| bits.read(self.bits))
            .collect::<Result<Vec<_>, _>>()?;
        bits.align();
        let widths = (0..c.groups)
            .map(|_| Ok(c.width_ref + bits.read(c.width_bits)?))
            .collect::<Result<Vec<_>, GribError>>()?;
        bits.align();
        let mut lengths = (0..c.groups)
            .map(|_| {
                let scaled = u64::from(bits.read(c.length_bits)?);
                Ok(u64::from(c.length_ref) + scaled * u64::from(c.length_inc))
            })
            .collect::<Result<Vec<_>, GribError>>()?;
        bits.align();
        if let Some(last) = lengths.last_mut() {
            *last = u64::from(c.last_length);
        }
        let total: u64 = lengths.iter().sum();
        if total != self.count as u64 {
            return Err(GribError::Invalid(format!(
                "groupes de {total} valeurs pour {}",
                self.count
            )));
        }

        let all_ones = |w: u32| -> u32 {
            if w >= 32 {
                u32::MAX
            } else {
                (1u32 << w) - 1
            }
        };
        let is_missing = |raw: u32, width: u32| -> bool {
            width > 0
                && ((c.missing >= 1 && raw == all_ones(width))
                    || (c.missing == 2 && raw == all_ones(width) - 1))
        };
        let mut ints: Vec<Option<i64>> = Vec::with_capacity(self.count);
        for ((&reference, &width), &length) in refs.iter().zip(&widths).zip(&lengths) {
            if width > 32 {
                return Err(GribError::Invalid(format!("groupe de {width} bits")));
            }
            if width == 0 {
                let value = if is_missing(reference, self.bits) {
                    None
                } else {
                    Some(i64::from(reference))
                };
                ints.extend(std::iter::repeat(value).take(length as usize));
            } else {
                for _ in 0..length {
                    let raw = bits.read(width)?;
                    ints.push(if is_missing(raw, width) {
                        None
                    } else {
                        Some(i64::from(reference) + i64::from(raw))
                    });
                }
            }
        }

        if let Some((order, _)) = c.differencing {
            // Les différences portent sur la suite des valeurs présentes.
            let mut present: Vec<&mut i64> = ints.iter_mut().flatten().collect();
            let order = usize::from(order);
            for (k, v) in present.iter_mut().enumerate().take(order) {
                **v = first[k];
            }
            for k in order..present.len() {
                let previous = *present[k - 1];
                let value = if order == 1 {
                    *present[k] + minimum + previous
                } else {
                    *present[k] + minimum + 2 * previous - *present[k - 2]
                };
                *present[k] = value;
            }
        }
        Ok(ints.into_iter().map(|v| v.map(|x| self.scale(x))).collect())
    }
}

/// Place les valeurs présentes sur la grille selon le bitmap.
fn apply_bitmap(
    packed: Vec<Option<f32>>,
    bitmap: Option<&[u8]>,
    npoints: usize,
) -> Result<Vec<Option<f32>>, GribError> {
    let Some(bitmap) = bitmap else {
        if packed.len() != npoints {
            return Err(GribError::Invalid(format!(
                "{} valeurs pour {npoints} points",
                packed.len()
            )));
        }
        return Ok(packed);
    };
    if bitmap.len() * 8 < npoints {
        return Err(GribError::Truncated("bitmap"));
    }
    let mut values = packed.into_iter();
    let mut out = Vec::with_capacity(npoints);
    for k in 0..npoints {
        if bitmap[k / 8] & (0x80 >> (k % 8)) != 0 {
            out.push(
                values
                    .next()
                    .ok_or_else(|| GribError::Invalid("bitmap : valeurs manquantes".to_owned()))?,
            );
        } else {
            out.push(None);
        }
    }
    if values.next().is_some() {
        return Err(GribError::Invalid("bitmap : valeurs en trop".to_owned()));
    }
    Ok(out)
}

fn find(haystack: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    haystack
        .get(from..)?
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + from)
}

/// Décode toutes les données GRIB2 d'un fichier.
///
/// # Errors
///
/// [`GribError`] si le fichier est trop volumineux, ne contient aucun
/// message, ou si un message est tronqué ou corrompu.
pub fn decode(bytes: &[u8]) -> Result<Decoded, GribError> {
    if bytes.len() > MAX_FILE_BYTES {
        return Err(GribError::TooLarge(bytes.len()));
    }
    let mut out = Decoded::default();
    let mut messages = 0usize;
    let mut pos = 0usize;
    while let Some(start) = find(bytes, pos, b"GRIB") {
        let header = bytes
            .get(start..start + 16)
            .ok_or(GribError::Truncated("section 0"))?;
        messages += 1;
        if header[7] != 2 {
            // GRIB1 : longueur sur 3 octets.
            let len = (usize::from(header[4]) << 16)
                | (usize::from(header[5]) << 8)
                | usize::from(header[6]);
            out.skipped
                .push(format!("message GRIB{} non pris en charge", header[7]));
            pos = start + len.max(8);
            continue;
        }
        let total = u64::from_be_bytes([
            header[8], header[9], header[10], header[11], header[12], header[13], header[14],
            header[15],
        ]);
        let total = usize::try_from(total).map_err(|_| GribError::TooLarge(usize::MAX))?;
        let end = start
            .checked_add(total)
            .filter(|&e| e <= bytes.len() && total >= 20)
            .ok_or(GribError::Truncated("message"))?;
        let message = &bytes[start..end];
        if &message[total - 4..] != b"7777" {
            return Err(GribError::Invalid(
                "fin de message « 7777 » absente".to_owned(),
            ));
        }
        decode_message(message, header[6], &mut out)?;
        pos = end;
    }
    if messages == 0 {
        return Err(GribError::NoMessage);
    }
    Ok(out)
}

fn decode_message(message: &[u8], discipline: u8, out: &mut Decoded) -> Result<(), GribError> {
    let mut reference: Option<DateTime<Utc>> = None;
    let mut grid: Option<Result<LatLonGrid, String>> = None;
    let mut product: Option<Result<Product, String>> = None;
    let mut packing: Option<Result<Packing, String>> = None;
    let mut bitmap: Option<&[u8]> = None;
    let mut previous_bitmap: Option<&[u8]> = None;
    let mut off = 16usize;
    let body_end = message.len() - 4;
    while off < body_end {
        let len = u32_at(message, off)? as usize;
        let section = off
            .checked_add(len)
            .filter(|&e| len >= 5 && e <= body_end)
            .map(|e| &message[off..e])
            .ok_or(GribError::Truncated("section"))?;
        match section[4] {
            1 => reference = Some(parse_reference_time(section)?),
            2 => {}
            3 => grid = Some(parse_grid(section)?),
            4 => product = Some(parse_product(section)?),
            5 => packing = Some(parse_packing(section)?),
            6 => {
                bitmap =
                    match u8_at(section, 5)? {
                        0 => {
                            previous_bitmap = Some(&section[6..]);
                            previous_bitmap
                        }
                        254 => Some(previous_bitmap.ok_or_else(|| {
                            GribError::Invalid("bitmap précédent absent".to_owned())
                        })?),
                        255 => None,
                        other => {
                            return Err(GribError::Invalid(format!("bitmap prédéfini {other}")));
                        }
                    };
            }
            7 => {
                let reference =
                    reference.ok_or_else(|| GribError::Invalid("section 1 absente".to_owned()))?;
                let (Some(grid), Some(product), Some(packing)) = (&grid, &product, &packing) else {
                    return Err(GribError::Invalid("sections 3 à 5 absentes".to_owned()));
                };
                match (grid, product, packing) {
                    (Ok(grid), Ok(product), Ok(packing)) => {
                        let packed = packing.unpack(&section[5..])?;
                        let values = apply_bitmap(packed, bitmap, grid.len())?;
                        out.fields.push(Field {
                            discipline,
                            category: product.category,
                            number: product.number,
                            surface_type: product.surface_type,
                            surface_value: product.surface_value,
                            reference_time: reference,
                            valid_time: reference + product.forecast,
                            grid: grid.clone(),
                            values,
                        });
                    }
                    (Err(reason), _, _) | (_, Err(reason), _) | (_, _, Err(reason)) => {
                        out.skipped.push(reason.clone());
                    }
                }
            }
            other => return Err(GribError::Invalid(format!("section {other}"))),
        }
        off += len;
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Écrivain de bits (tests : fabrication de messages).
    #[derive(Default)]
    pub(crate) struct BitWriter {
        bytes: Vec<u8>,
        bit: usize,
    }

    impl BitWriter {
        pub(crate) fn put(&mut self, value: u64, n: u32) {
            for k in (0..n).rev() {
                if self.bit % 8 == 0 {
                    self.bytes.push(0);
                }
                if (value >> k) & 1 == 1 {
                    let last = self.bytes.len() - 1;
                    self.bytes[last] |= 0x80 >> (self.bit % 8);
                }
                self.bit += 1;
            }
        }

        pub(crate) fn put_signed(&mut self, value: i64, n: u32) {
            self.put(u64::from(value < 0), 1);
            self.put(value.unsigned_abs(), n - 1);
        }

        pub(crate) fn align(&mut self) {
            self.bit = self.bit.div_ceil(8) * 8;
        }

        pub(crate) fn finish(self) -> Vec<u8> {
            self.bytes
        }
    }

    fn section(number: u8, body: &[u8]) -> Vec<u8> {
        let mut s = ((body.len() + 5) as u32).to_be_bytes().to_vec();
        s.push(number);
        s.extend_from_slice(body);
        s
    }

    fn sm32(v: i64) -> [u8; 4] {
        let raw = v.unsigned_abs() as u32 | if v < 0 { 0x8000_0000 } else { 0 };
        raw.to_be_bytes()
    }

    /// Paramètres d'un message de test.
    pub(crate) struct Spec {
        pub category: u8,
        pub number: u8,
        pub forecast_h: u32,
        pub height_m: u32,
        pub ni: u32,
        pub nj: u32,
        /// Premier point (micro-degrés) et pas.
        pub lat1: i64,
        pub lon1: i64,
        pub step: u32,
        pub scan: u8,
    }

    impl Default for Spec {
        fn default() -> Self {
            Self {
                category: 2,
                number: 2,
                forecast_h: 3,
                height_m: 10,
                ni: 3,
                nj: 2,
                lat1: -14_000_000,
                lon1: 210_000_000,
                step: 500_000,
                scan: 0,
            }
        }
    }

    pub(crate) fn sections_1_to_4(spec: &Spec) -> Vec<u8> {
        let mut m = Vec::new();
        // Section 1 : référence 2026-11-12 06:00:00 UTC.
        let mut s1 = vec![0u8; 16];
        s1[7..9].copy_from_slice(&2026u16.to_be_bytes());
        s1[9] = 11;
        s1[10] = 12;
        s1[11] = 6;
        m.extend(section(1, &s1));
        // Section 3 : grille 3.0.
        let mut s3 = vec![0u8; 67];
        let n = spec.ni * spec.nj;
        s3[1..5].copy_from_slice(&n.to_be_bytes());
        s3[7..9].copy_from_slice(&0u16.to_be_bytes());
        s3[9] = 6;
        s3[25..29].copy_from_slice(&spec.ni.to_be_bytes());
        s3[29..33].copy_from_slice(&spec.nj.to_be_bytes());
        s3[41..45].copy_from_slice(&sm32(spec.lat1));
        s3[45..49].copy_from_slice(&sm32(spec.lon1));
        s3[49] = 0x30;
        let lat2 = spec.lat1 - i64::from(spec.step) * i64::from(spec.nj - 1);
        let lon2 = spec.lon1 + i64::from(spec.step) * i64::from(spec.ni - 1);
        s3[50..54].copy_from_slice(&sm32(lat2));
        s3[54..58].copy_from_slice(&sm32(lon2));
        s3[58..62].copy_from_slice(&spec.step.to_be_bytes());
        s3[62..66].copy_from_slice(&spec.step.to_be_bytes());
        s3[66] = spec.scan;
        m.extend(section(3, &s3));
        // Section 4 : produit 4.0.
        let mut s4 = vec![0u8; 29];
        s4[4] = spec.category;
        s4[5] = spec.number;
        s4[12] = 1; // heures
        s4[13..17].copy_from_slice(&spec.forecast_h.to_be_bytes());
        s4[17] = 103;
        s4[19..23].copy_from_slice(&spec.height_m.to_be_bytes());
        s4[23] = 255;
        m.extend(section(4, &s4));
        m
    }

    /// Section 5 compactage simple : `R + X × 2^E` / 10^D.
    pub(crate) fn simple_packing(count: u32, reference: f32, e: i16, d: i16, bits: u8) -> Vec<u8> {
        let mut s5 = vec![0u8; 16];
        s5[0..4].copy_from_slice(&count.to_be_bytes());
        s5[4..6].copy_from_slice(&0u16.to_be_bytes());
        s5[6..10].copy_from_slice(&reference.to_bits().to_be_bytes());
        let sm16 = |v: i16| (v.unsigned_abs() | if v < 0 { 0x8000 } else { 0 }).to_be_bytes();
        s5[10..12].copy_from_slice(&sm16(e));
        s5[12..14].copy_from_slice(&sm16(d));
        s5[14] = bits;
        section(5, &s5)
    }

    pub(crate) fn message(discipline: u8, body: Vec<u8>) -> Vec<u8> {
        let total = (16 + body.len() + 4) as u64;
        let mut m = b"GRIB".to_vec();
        m.extend([0, 0, discipline, 2]);
        m.extend(total.to_be_bytes());
        m.extend(body);
        m.extend(b"7777");
        m
    }

    /// Message vent simple : valeurs entières (m/s × 10 ⇒ D = 1).
    pub(crate) fn simple_message(spec: &Spec, raw: &[u32], bitmap: Option<Vec<u8>>) -> Vec<u8> {
        let mut body = sections_1_to_4(spec);
        body.extend(simple_packing(raw.len() as u32, -50.0, 0, 1, 12));
        match bitmap {
            Some(b) => {
                let mut s6 = vec![0u8];
                s6.extend(b);
                body.extend(section(6, &s6));
            }
            None => body.extend(section(6, &[255])),
        }
        let mut w = BitWriter::default();
        for &x in raw {
            w.put(u64::from(x), 12);
        }
        body.extend(section(7, &w.finish()));
        message(0, body)
    }

    #[test]
    fn decodes_simple_packing() {
        let raw = [500, 510, 520, 530, 540, 1000];
        let decoded = decode(&simple_message(&Spec::default(), &raw, None)).expect("decode");
        assert!(decoded.skipped.is_empty());
        let f = &decoded.fields[0];
        assert_eq!((f.discipline, f.category, f.number), (0, 2, 2));
        assert_eq!((f.surface_type, f.surface_value), (103, 10.0));
        assert_eq!(f.reference_time.to_rfc3339(), "2026-11-12T06:00:00+00:00");
        assert_eq!(f.valid_time.to_rfc3339(), "2026-11-12T09:00:00+00:00");
        // (−50 + X) / 10.
        assert_eq!(f.values[0], Some(45.0));
        assert_eq!(f.values[5], Some(95.0));
        assert_eq!(f.grid.point(0, 0), (-14.0, -150.0));
        assert_eq!(f.grid.point(2, 1), (-14.5, -149.0));
    }

    #[test]
    fn applies_bitmap() {
        let raw = [600, 700];
        let decoded = decode(&simple_message(
            &Spec::default(),
            &raw,
            Some(vec![0b1000_0100]),
        ))
        .expect("ok");
        let v = &decoded.fields[0].values;
        assert_eq!(v, &[Some(55.0), None, None, None, None, Some(65.0)]);
        // Bitmap incohérent : erreur, pas de valeurs inventées.
        let bad = simple_message(&Spec::default(), &raw, Some(vec![0b1110_0000]));
        assert!(decode(&bad).is_err());
    }

    #[test]
    fn decodes_complex_packing_with_second_order_differencing() {
        // Valeurs entières à retrouver (X), puis R = 0, D = 0.
        let x: [i64; 6] = [100, 103, 108, 110, 109, 120];
        // Différences d'ordre 2 : d_k = x_k − 2 x_{k−1} + x_{k−2} (k ≥ 2).
        let d: Vec<i64> = (2..6).map(|k| x[k] - 2 * x[k - 1] + x[k - 2]).collect();
        let gmin = *d.iter().min().expect("min");
        // Valeurs compactées : 0, 0 (remplacées), puis d − gmin.
        let packed: Vec<i64> = [0, 0]
            .into_iter()
            .chain(d.iter().map(|v| v - gmin))
            .collect();
        // Deux groupes : [0, 0, a] largeur 4 et [b, c, e] largeur 4, référence = min.
        let groups = [&packed[0..3], &packed[3..6]];
        let refs: Vec<i64> = groups.iter().map(|g| *g.iter().min().expect("m")).collect();

        let mut body = sections_1_to_4(&Spec::default());
        let mut s5 = vec![0u8; 44];
        s5[0..4].copy_from_slice(&6u32.to_be_bytes());
        s5[4..6].copy_from_slice(&3u16.to_be_bytes());
        s5[6..10].copy_from_slice(&0f32.to_bits().to_be_bytes());
        s5[14] = 8; // bits des références de groupe
        s5[16] = 1; // groupes généraux
        s5[17] = 0; // pas de valeurs absentes
        s5[26..30].copy_from_slice(&2u32.to_be_bytes()); // NG
        s5[30] = 0; // référence des largeurs
        s5[31] = 4; // bits des largeurs
        s5[32..36].copy_from_slice(&3u32.to_be_bytes()); // référence des longueurs
        s5[36] = 1; // incrément
        s5[37..41].copy_from_slice(&3u32.to_be_bytes()); // longueur du dernier groupe
        s5[41] = 1; // bits des longueurs
        s5[42] = 2; // ordre 2
        s5[43] = 2; // descripteurs sur 2 octets
        body.extend(section(5, &s5));
        body.extend(section(6, &[255]));
        let mut w = BitWriter::default();
        w.put_signed(x[0], 16);
        w.put_signed(x[1], 16);
        w.put_signed(gmin, 16);
        w.align();
        for r in &refs {
            w.put(*r as u64, 8);
        }
        w.align();
        for _ in 0..2 {
            w.put(4, 4); // largeur 4
        }
        w.align();
        for _ in 0..2 {
            w.put(0, 1); // longueur 3 + 0
        }
        w.align();
        for (g, r) in groups.iter().zip(&refs) {
            for v in g.iter() {
                w.put((v - r) as u64, 4);
            }
        }
        body.extend(section(7, &w.finish()));
        let decoded = decode(&message(0, body)).expect("decode");
        let values: Vec<f32> = decoded.fields[0]
            .values
            .iter()
            .map(|v| v.expect("v"))
            .collect();
        assert_eq!(values, x.map(|v| v as f32).to_vec());
    }

    #[test]
    fn skips_unsupported_packing_without_failing() {
        let mut body = sections_1_to_4(&Spec::default());
        let mut s5 = simple_packing(6, 0.0, 0, 0, 8);
        s5[9..11].copy_from_slice(&40u16.to_be_bytes()); // JPEG2000
        body.extend(s5);
        body.extend(section(6, &[255]));
        body.extend(section(7, &[0; 6]));
        let mut file = message(0, body);
        file.extend(simple_message(&Spec::default(), &[1, 2, 3, 4, 5, 6], None));
        let decoded = decode(&file).expect("decode");
        assert_eq!(decoded.fields.len(), 1);
        assert_eq!(decoded.skipped, vec!["compactage 5.40".to_owned()]);
    }

    #[test]
    fn rejects_malformed_data() {
        assert_eq!(decode(b"rien a voir"), Err(GribError::NoMessage));
        let good = simple_message(&Spec::default(), &[1, 2, 3, 4, 5, 6], None);
        // Tronqué.
        assert!(decode(&good[..good.len() - 10]).is_err());
        // Fin « 7777 » corrompue.
        let mut bad = good.clone();
        let n = bad.len();
        bad[n - 1] = b'8';
        assert!(decode(&bad).is_err());
        // Longueur de section aberrante.
        let mut bad = good.clone();
        bad[16..20].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(decode(&bad).is_err());
        // Données trop courtes pour le nombre de valeurs annoncé.
        let mut body = sections_1_to_4(&Spec::default());
        body.extend(simple_packing(6, 0.0, 0, 0, 12));
        body.extend(section(6, &[255]));
        body.extend(section(7, &[0; 2]));
        assert!(decode(&message(0, body)).is_err());
        // Aucune panique sur des octets quelconques.
        for cut in 0..good.len() {
            let _ = decode(&good[..cut]);
        }
        let mut noise = good.clone();
        for (k, b) in noise.iter_mut().enumerate().skip(16) {
            *b = b.wrapping_mul(31).wrapping_add(k as u8);
        }
        let _ = decode(&noise);
    }

    #[test]
    fn handles_scanning_directions() {
        let spec = Spec {
            scan: 0x40,
            lat1: -15_000_000,
            ..Spec::default()
        };
        let f = decode(&simple_message(&spec, &[0; 6], None))
            .expect("ok")
            .fields
            .remove(0);
        assert_eq!(f.grid.point(0, 1), (-14.5, -150.0));
        let spec = Spec {
            scan: 0x20,
            ..Spec::default()
        };
        let decoded = decode(&simple_message(&spec, &[0; 6], None)).expect("ok");
        assert!(decoded.fields.is_empty());
        assert_eq!(decoded.skipped, vec!["balayage 0x20".to_owned()]);
    }
}
