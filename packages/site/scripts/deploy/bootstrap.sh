#!/usr/bin/env bash
set -euo pipefail
[[ $EUID == 0 ]] || { echo 'Deployment requires a trusted root SSH target.' >&2; exit 1; }
. /etc/os-release
[[ $ID == ubuntu || $ID == debian ]] || { echo 'Ubuntu or Debian is required.' >&2; exit 1; }
install -d -m 700 /etc/rustier-site
exec 9>/etc/rustier-site/deploy.lock
flock -n 9 || { echo 'Another deployment is running.' >&2; exit 1; }

missing=()
for tool in node rsync curl openssl; do
	command -v "$tool" >/dev/null || missing+=("${tool/node/nodejs}")
done
if ((${#missing[@]})); then
	apt-get update >&2
	DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends "${missing[@]}" ca-certificates >&2
fi
if ! command -v docker >/dev/null || ! docker compose version >/dev/null 2>&1; then
	apt-get update >&2
	DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends ca-certificates curl >&2
	install -d -m 755 /etc/apt/keyrings
	curl --proto '=https' --tlsv1.2 -fsSL "https://download.docker.com/linux/$ID/gpg" -o /etc/apt/keyrings/rustier-docker.asc
	chmod 644 /etc/apt/keyrings/rustier-docker.asc
	echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/rustier-docker.asc] https://download.docker.com/linux/$ID ${VERSION_CODENAME} stable" > /etc/apt/sources.list.d/rustier-docker.list
	apt-get update >&2
	DEBIAN_FRONTEND=noninteractive apt-get install -y docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin >&2
fi
systemctl is-active --quiet docker || systemctl enable --now docker >&2
docker info >/dev/null

# Preserve the stronger Cloudflare-only rules of the historical deployment.
if systemctl is-active --quiet rustier-firewall.service && nft list table inet rustier_site >/dev/null 2>&1; then
	echo 'Dependencies and existing Rustier firewall are ready.' >&2
	exit 0
fi
if ! command -v ufw >/dev/null; then
	apt-get update >&2
	DEBIAN_FRONTEND=noninteractive apt-get install -y ufw >&2
fi
if ufw status | grep -q '^Status: active'; then
	echo 'Existing active firewall preserved.' >&2
	exit 0
fi
ssh_port=${SSH_CONNECTION##* }
[[ $ssh_port =~ ^[0-9]+$ ]] || { echo 'Cannot identify the active SSH port; refusing to enable a firewall.' >&2; exit 1; }
ufw allow "$ssh_port/tcp" >&2
ufw allow 80/tcp >&2
ufw allow 443/tcp >&2
ufw default deny incoming >&2
ufw default allow outgoing >&2
# A fresh SSH connection confirms access before cancelling this rollback timer.
systemd-run --unit=rustier-ufw-rollback --on-active=120s /usr/sbin/ufw --force disable >&2
ufw --force enable >&2
