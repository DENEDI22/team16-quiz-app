# Quiz-App auf einem 2-Node k3s-Cluster (2× Raspberry Pi)

Ziel: Die Quiz-App auf einem Cluster aus 2 Raspberry Pis betreiben. Diese
Anleitung lässt dich das Setup zuerst lokal üben und führt dann durch die echte
Pi-Installation. Die Manifeste liegen in `k8s/` und werden in jedem Szenario mit
`kubectl apply -k k8s/` ausgerollt.

## 1. Überblick

k3s ist eine schlanke Kubernetes-Distribution als Einzelbinary. Ein Cluster
besteht aus:

- **1 Server-Node** (`pi-server`) — Control Plane (API-Server, Scheduler,
  SQLite). Auf einem 2-Pi-Cluster läuft er auch als Worker und nimmt Workloads auf.
- **1 Agent-Node** (`pi-agent1`) — Worker, der dem Server über ein gemeinsames
  Token beitritt.

Was k3s out-of-the-box mitbringt und wir nutzen:

- **Traefik** — Ingress-Controller, der auf Port 80/443 jedes Nodes lauscht und
  Requests nach Pfad routet (siehe `k8s/ingress.yaml`):
  `/api/auth` → auth-service, `/api/singleplayer` → singleplayer-service,
  `/api/multiplayer` → multiplayer-service, `/api/scoreboard` → scoreboard-service,
  alles andere → frontend.
- **local-path StorageClass** — Standard-StorageClass, damit der Postgres-PVC
  ohne weiteres Zutun gebunden wird.

> **Ressourcen-Hinweis:** Mit 2 Nodes verteilt der Scheduler die Pods auf beide
> Maschinen, kann aber keine harte Trennung garantieren. Auf einem Pi 4 (4 GB RAM)
> laufen alle Services erfahrungsgemäß; auf einem Pi 4 mit 2 GB kann es eng
> werden. Pi 5 hat mehr Reserve.

---

## 2. Lokal üben mit k3d (empfohlener Start)

k3d betreibt jeden k3s-Node als Docker-Container — schnellster Weg, um die
Manifeste zu testen, bevor du die echten Pis anfasst.

### Voraussetzungen

```bash
# Arch
sudo pacman -S kubectl docker
yay -S k3d
# oder: curl -s https://raw.githubusercontent.com/k3d-io/k3d/main/install.sh | bash
```

### 2-Node-Cluster anlegen

```bash
# 1 Server + 1 Agent, Port 8081 → Traefik-Port 80
k3d cluster create quiz --servers 1 --agents 1 -p "8081:80@loadbalancer"

kubectl get nodes   # sollte 2 Nodes zeigen, kubeconfig wird automatisch gesetzt
```

### Images bauen und in den Cluster laden

Der Cluster sieht deine lokalen Docker-Images nicht — daher bauen und importieren:

```bash
# Backend-Services (vom Repo-Root aus)
for s in auth quiz scoreboard singleplayer multiplayer; do
  docker build -t quiz-app-$s-service:dev \
    -f backend/crates/$s-service/dockerfile backend/
done

# Frontend
docker build -t quiz-app-frontend:dev frontend/team16-quiz-app/

k3d image import -c quiz \
  quiz-app-auth-service:dev quiz-app-quiz-service:dev \
  quiz-app-scoreboard-service:dev quiz-app-singleplayer-service:dev \
  quiz-app-multiplayer-service:dev quiz-app-frontend:dev
```

Passe danach den `images:`-Block in `k8s/kustomization.yaml` an, damit
Kubernetes die lokalen Tags nutzt (kein Pull vom Hub):

```yaml
# k8s/kustomization.yaml — für lokales k3d
images:
  - name: quiz-app-auth-service
    newName: quiz-app-auth-service
    newTag: dev
  # … analog für alle anderen Services
```

> Mit einem `:dev`-Tag (nicht `:latest`) verhindert Kubernetes automatisch
> Pull-Versuche gegen Docker Hub.

### Deployen

```bash
kubectl apply -k k8s/
kubectl -n quiz-app get pods -w   # warten, bis alle Running/Ready sind
```

Die App ist jetzt unter **http://localhost:8081** erreichbar.

### Iterieren

```bash
# Einen Service neu bauen und ausrollen
docker build -t quiz-app-auth-service:dev -f backend/crates/auth-service/dockerfile backend/
k3d image import -c quiz quiz-app-auth-service:dev
kubectl -n quiz-app rollout restart deployment/auth-service

# Cluster wegwerfen und neu starten
k3d cluster delete quiz
```

---

## 3. VMs zum Rehearsen (optional)

Dieselben Befehle, die auf den Pis laufen, in lokalen VMs üben:

```bash
yay -S multipass
multipass launch -n pi-server -c 2 -m 2G -d 8G
multipass launch -n pi-agent1 -c 1 -m 1G -d 5G
```

**Auf `pi-server`** (`multipass shell pi-server`):

```bash
curl -sfL https://get.k3s.io | sh -
sudo cat /var/lib/rancher/k3s/server/node-token   # Token für den Agent
ip -4 addr show                                    # IP des Servers
```

**Auf `pi-agent1`** (`multipass shell pi-agent1`):

```bash
curl -sfL https://get.k3s.io | \
  K3S_URL=https://<SERVER_IP>:6443 \
  K3S_TOKEN=<NODE_TOKEN> sh -
```

**Auf deiner Maschine** — kubeconfig holen:

```bash
multipass exec pi-server -- sudo cat /etc/rancher/k3s/k3s.yaml > ~/.kube/quiz-cluster.yaml
# 127.0.0.1 durch die Server-IP ersetzen:
sed -i 's/127\.0\.0\.1/<SERVER_IP>/' ~/.kube/quiz-cluster.yaml
export KUBECONFIG=~/.kube/quiz-cluster.yaml
kubectl get nodes   # sollte 2 Nodes zeigen
```

Deployment: Da VMs keine `k3d image import`-Möglichkeit haben, müssen Images
von Docker Hub kommen (siehe Abschnitt 4.4).

---

## 4. Das echte 2-Pi-Setup

### 4.1 OS flashen

Empfohlen: **Raspberry Pi OS Lite (64-bit)** oder **Ubuntu Server 24.04 arm64**.
Mit dem Raspberry Pi Imager vorab konfigurieren:

- SSH aktivieren
- Hostname: `pi-server` (Server-Pi) bzw. `pi-agent1` (Worker-Pi)
- Benutzer + Passwort setzen

### 4.2 Statische IPs

Damit der Agent den Server immer unter derselben Adresse erreicht, entweder:

- **DHCP-Reservation im Router** (empfohlen, kein Eingriff ins OS nötig) oder
- Statische IP direkt auf dem Pi konfigurieren.

### 4.3 cgroups aktivieren (nur Raspberry Pi OS)

k3s startet ohne Memory-cgroups nicht. In `/boot/firmware/cmdline.txt` am Ende
der **einzigen** Zeile anfügen (kein Zeilenumbruch einfügen):

```
cgroup_memory=1 cgroup_enable=memory
```

Danach den Pi neu starten. Ubuntu Server braucht diesen Schritt nicht.

### 4.4 System aktualisieren

Auf beiden Pis:

```bash
sudo apt update && sudo apt full-upgrade -y
sudo swapoff -a && sudo systemctl disable dphys-swapfile   # Swap deaktivieren
```

### 4.5 k3s installieren

**Auf `pi-server`** (per SSH):

```bash
curl -sfL https://get.k3s.io | sh -

# Token und IP für den Agent notieren:
sudo cat /var/lib/rancher/k3s/server/node-token
hostname -I | awk '{print $1}'
```

**Auf `pi-agent1`** (per SSH):

```bash
curl -sfL https://get.k3s.io | \
  K3S_URL=https://<PI_SERVER_IP>:6443 \
  K3S_TOKEN=<NODE_TOKEN> sh -
```

### 4.6 kubeconfig auf die Entwicklermaschine holen

```bash
scp pi@<PI_SERVER_IP>:/etc/rancher/k3s/k3s.yaml ~/.kube/quiz-cluster.yaml
# 127.0.0.1 → echte Server-IP ersetzen:
sed -i 's/127\.0\.0\.1/<PI_SERVER_IP>/' ~/.kube/quiz-cluster.yaml
export KUBECONFIG=~/.kube/quiz-cluster.yaml

kubectl get nodes   # sollte pi-server + pi-agent1 zeigen
```

---

## 5. App deployen

### 5.1 Docker Hub — Images bauen und pushen

Die Pis können keine lokalen Images importieren. Die Images müssen für `arm64`
gebaut und auf Docker Hub gepusht sein. Entweder manuell oder via CI.

**Manuell (einmalig):**

```bash
# Ersetze DEIN_USERNAME durch deinen Docker Hub Account
export DOCKER_USER=DEIN_USERNAME

for s in auth quiz scoreboard singleplayer multiplayer; do
  docker buildx build --platform linux/arm64 \
    -t $DOCKER_USER/quiz-app-$s-service:latest \
    -f backend/crates/$s-service/dockerfile backend/ --push
done

docker buildx build --platform linux/arm64 \
  -t $DOCKER_USER/quiz-app-frontend:latest \
  frontend/team16-quiz-app/ --push
```

> `docker buildx` mit `linux/arm64`-Target benötigt QEMU-Emulation oder ein
> natives arm64-Build-System. Auf einem amd64-Rechner einmalig einrichten:
> `docker run --privileged --rm tonistiigi/binfmt --install all`

### 5.2 kustomization.yaml anpassen

In `k8s/kustomization.yaml` den `CHANGEME`-Platzhalter durch deinen Docker Hub
Account ersetzen:

```yaml
images:
  - name: quiz-app-auth-service
    newName: docker.io/DEIN_USERNAME/quiz-app-auth-service
    newTag: latest
  # … analog für alle anderen Services
```

### 5.3 Secrets anlegen

Die Datei `k8s/secrets.yaml` enthält nur Dev-Platzhalter. Für den echten Cluster
echte Secrets erstellen und `secrets.yaml` aus `kustomization.yaml` entfernen:

```bash
kubectl create namespace quiz-app

kubectl -n quiz-app create secret generic quiz-app-secrets \
  --from-literal=JWT_SECRET="$(openssl rand -hex 32)" \
  --from-literal=POSTGRES_USER=admin \
  --from-literal=POSTGRES_PASSWORD="$(openssl rand -hex 16)"
```

Dann `secrets.yaml` aus der `resources:`-Liste in `k8s/kustomization.yaml`
entfernen (damit der `kubectl apply` das Dev-Secret nicht überschreibt).

### 5.4 Ausrollen

```bash
kubectl apply -k k8s/
kubectl -n quiz-app get pods -w   # warten, bis alle Running/Ready sind
```

---

## 6. App extern erreichbar machen (ngrok)

Traefik lauscht auf Port 80 **jedes** Nodes. ngrok auf einem der Pis oder einer
Maschine im selben Netz starten, die einen Node erreicht:

```bash
# Auf pi-server oder pi-agent1:
ngrok http 80
```

Ein Tunnel, ein Einstiegspunkt — Frontend, REST-API und WebSockets laufen alle
darüber. Da das Frontend relative `/api/...`-URLs und `wss://` aus `location`
ableitet, ist kein Rebuild für die ngrok-Domain nötig.

---

## 7. Troubleshooting-Referenz

| Symptom | Erster Befehl | Häufige Ursache |
|---|---|---|
| Pod `Pending` | `kubectl -n quiz-app describe pod <pod>` | Kein Node mit freien Ressourcen / PVC nicht gebunden |
| Pod `ImagePullBackOff` | wie oben | Image-Name/Tag falsch, nicht gepusht, oder falsche Architektur (`amd64` statt `arm64`) |
| Pod `CrashLoopBackOff` | `kubectl -n quiz-app logs <pod> --previous` | Fehlende Env-Variable (Rust-Services `expect()` sie), DB noch nicht bereit |
| 404 von Traefik | `kubectl -n quiz-app get ingress,middleware` | Middleware-Annotation/Namespace-Prefix falsch |
| Service nicht erreichbar | `kubectl -n quiz-app get endpoints` | Label-Selector stimmt nicht mit Pod-Labels überein, oder Readiness-Probe schlägt fehl |
| Postgres-DBs fehlen | `kubectl -n quiz-app exec -it deploy/postgres -- psql -U admin -l` | Init-Skript läuft nur bei **leerem** Volume — PVC löschen zum Neu-Initialisieren |
| k3s startet nicht (Pi) | `sudo journalctl -u k3s -f` | cgroups nicht aktiviert (nur Raspberry Pi OS) |
| Agent joint nicht | `sudo journalctl -u k3s-agent -f` | TOKEN oder SERVER_IP falsch, oder Firewall blockiert Port 6443 |

Empfehlenswert: `k9s` als Terminal-UI für den Cluster (`sudo apt install k9s`
oder Binary von GitHub).
