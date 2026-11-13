#!/usr/bin/env bash
# Construit le paquet Debian de nautikd (Raspberry Pi OS, Debian, Ubuntu).
#
# Usage : apps/daemon/packaging/build-deb.sh <binaire nautikd> <arm64|amd64> [dossier de sortie]
# Dépendance : dpkg-deb.
#
# Le paquet installe le binaire dans /usr/bin, l'unité systemd et la
# documentation ; il crée le compte de service « nautik » (groupe dialout
# pour les ports série) puis active et démarre le service. L'API n'écoute
# que sur la machine (127.0.0.1:8375) : aucune exposition réseau
# automatique. La désinstallation (même « purge ») conserve les données
# de /var/lib/nautik (traces, routes, jeton).
set -euo pipefail

BIN="${1:?binaire nautikd requis}"
ARCH="${2:?architecture requise (arm64 ou amd64)}"
OUT="${3:-.}"
case "$ARCH" in arm64|amd64) ;; *) echo "architecture non prise en charge : $ARCH" >&2; exit 2 ;; esac
[ -x "$BIN" ] || { echo "binaire introuvable ou non exécutable : $BIN" >&2; exit 2; }

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
VERSION="$(sed -n 's/^version = "\(.*\)"$/\1/p' "$ROOT/Cargo.toml" | head -n 1)"
[ -n "$VERSION" ] || { echo "version introuvable dans Cargo.toml" >&2; exit 1; }

STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT
PKG="$STAGE/nautikd_${VERSION}_${ARCH}"

install -D -m 0755 "$BIN" "$PKG/usr/bin/nautikd"
# Unité de référence, binaire placé dans /usr/bin par le paquet.
install -d "$PKG/lib/systemd/system"
sed 's#^ExecStart=/usr/local/bin/nautikd#ExecStart=/usr/bin/nautikd#' \
  "$ROOT/apps/daemon/systemd/nautikd.service" > "$PKG/lib/systemd/system/nautikd.service"
chmod 0644 "$PKG/lib/systemd/system/nautikd.service"
grep -q '^ExecStart=/usr/bin/nautikd$' "$PKG/lib/systemd/system/nautikd.service" \
  || { echo "ExecStart non adapté dans l'unité systemd" >&2; exit 1; }
install -D -m 0644 "$ROOT/apps/daemon/README.md" "$PKG/usr/share/doc/nautikd/README.md"

install -d "$PKG/DEBIAN"
cat > "$PKG/DEBIAN/control" <<EOF
Package: nautikd
Version: ${VERSION}
Architecture: ${ARCH}
Section: misc
Priority: optional
Depends: libc6, adduser, systemd
Maintainer: NautiK <nautik@invalid>
Homepage: https://github.com/laurentpellet/NautiK
Description: NautiK - service de navigation autonome
 Moteur de navigation sans interface graphique : acquisition Signal K et
 NMEA 0183, AIS, routes, traces et alarmes, API REST/WebSocket locale
 protégée par jeton. Aide à la navigation, pas un ECDIS certifié.
EOF

cat > "$PKG/DEBIAN/postinst" <<'EOF'
#!/bin/sh
set -e
if [ "$1" = "configure" ]; then
    if ! getent passwd nautik >/dev/null; then
        adduser --system --group --no-create-home --home /var/lib/nautik \
            --shell /usr/sbin/nologin nautik
    fi
    if getent group dialout >/dev/null; then
        adduser nautik dialout >/dev/null
    fi
    if [ -d /run/systemd/system ]; then
        systemctl daemon-reload
        systemctl enable nautikd.service >/dev/null
        systemctl restart nautikd.service
    fi
fi
exit 0
EOF

cat > "$PKG/DEBIAN/prerm" <<'EOF'
#!/bin/sh
set -e
if [ -d /run/systemd/system ] && [ "$1" = "remove" ]; then
    systemctl disable --now nautikd.service >/dev/null || true
fi
exit 0
EOF

cat > "$PKG/DEBIAN/postrm" <<'EOF'
#!/bin/sh
set -e
if [ -d /run/systemd/system ]; then
    systemctl daemon-reload || true
fi
# Les données de navigation (/var/lib/nautik) sont conservées, même en
# purge : supprimez-les à la main si vous n'en avez plus besoin.
exit 0
EOF
chmod 0755 "$PKG/DEBIAN/postinst" "$PKG/DEBIAN/prerm" "$PKG/DEBIAN/postrm"

mkdir -p "$OUT"
dpkg-deb --root-owner-group --build "$PKG" "$OUT/nautikd_${VERSION}_${ARCH}.deb"
echo "Paquet : $OUT/nautikd_${VERSION}_${ARCH}.deb"
