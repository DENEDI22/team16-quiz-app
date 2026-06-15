# k3s: Practicing Locally, Deploying on the Pi Cluster

Goal: run the quiz app on a 4-node k3s cluster (4× Raspberry Pi 5). This guide
lets you practice the whole thing on your dev machine first, then walks through
the real Pi setup. The app manifests live in `k8s/` and are deployed with
`kubectl apply -k k8s/` in every variant.

## 1. The big picture

k3s is a single-binary Kubernetes distribution. A cluster consists of:

- **1 server node** (control plane — runs the API server, scheduler, etcd/SQLite,
  *and* can run workloads; on a 4-Pi cluster you don't waste a Pi on it)
- **N agent nodes** (workers that join the server using a shared token)

Two things k3s ships out of the box that we rely on:

- **Traefik** — an ingress controller, i.e. the reverse proxy from our
  architecture. It listens on ports 80/443 on every node and routes by path
  (see `k8s/ingress.yaml`): `/api/auth` → auth-service, `/api/singleplayer` →
  singleplayer-service, `/api/multiplayer` → multiplayer-service, everything
  else → frontend. ngrok only needs to point at port 80.
- **local-path storage** — a default StorageClass, so the Postgres
  PersistentVolumeClaim just works.

There are two good ways to practice locally. **k3d** (Option A) is the fastest
way to get a multi-node cluster and iterate on manifests. **VMs** (Option B)
replicate the actual installation procedure you'll run on the Pis (the
commands are identical). Do A to learn Kubernetes/the app deployment, do B once
to rehearse the cluster bootstrap.

## 2. Option A — k3d: a 4-node cluster in Docker (recommended start)

k3d runs each k3s node as a Docker container.

```bash
# Arch: k3d and kubectl
sudo pacman -S kubectl
yay -S k3d        # or: curl -s https://raw.githubusercontent.com/k3d-io/k3d/main/install.sh | bash

# 1 server + 3 agents, mirroring the Pi topology.
# -p maps localhost:8081 to port 80 of Traefik inside the cluster.
k3d cluster create quiz --servers 1 --agents 3 -p "8081:80@loadbalancer"

kubectl get nodes   # should show 4 nodes, k3d sets the kubeconfig for you
```

### Build the images and load them into the cluster

The cluster can't see your local Docker images, so build and import them:

```bash
# backend services (from repo root)
for s in auth quiz scoreboard singleplayer multiplayer; do
  docker build -t quiz-app-$s-service:dev \
    -f backend/crates/$s-service/dockerfile backend/
done

# frontend
docker build -t quiz-app-frontend:dev frontend/team16-quiz-app/

k3d image import -c quiz \
  quiz-app-auth-service:dev quiz-app-quiz-service:dev \
  quiz-app-scoreboard-service:dev quiz-app-singleplayer-service:dev \
  quiz-app-multiplayer-service:dev quiz-app-frontend:dev
```

For local images, point the `images:` section in `k8s/kustomization.yaml` at
them (`newName: quiz-app-auth-service`, `newTag: dev`) — or keep a copy of the
file as a local overlay. For the real cluster you set `newName` to your Docker
Hub account instead and the nodes pull the images themselves.

> When using locally imported images, add `imagePullPolicy: IfNotPresent`
> semantics by using a non-`latest` tag (we use `:dev` above — `:latest` would
> make Kubernetes try to pull from Docker Hub and fail).

### Deploy

```bash
kubectl apply -k k8s/
kubectl -n quiz-app get pods -w        # wait until everything is Running/Ready
```

The app is now at **http://localhost:8081** — login, singleplayer (WebSocket),
multiplayer, everything through the single Traefik entry point.

### Iterate

```bash
# rebuild one service and roll it out
docker build -t quiz-app-auth-service:dev -f backend/crates/auth-service/dockerfile backend/
k3d image import -c quiz quiz-app-auth-service:dev
kubectl -n quiz-app rollout restart deployment/auth-service

# debugging essentials
kubectl -n quiz-app logs deploy/auth-service -f
kubectl -n quiz-app describe pod <pod>      # events: why is it Pending/CrashLooping?
kubectl -n quiz-app exec -it deploy/postgres -- psql -U admin -l

# throw it all away and start over (cheap — that's the point of practicing here)
k3d cluster delete quiz
```

## 3. Option B — VMs: rehearse the real installation

This is exactly what you'll type on the Pis, just in VMs. Any hypervisor works;
`multipass` is the least friction:

```bash
yay -S multipass   # Arch (AUR)
multipass launch -n k3s-server -c 2 -m 2G -d 8G
multipass launch -n k3s-agent1 -c 1 -m 1G -d 5G   # repeat for agent2, agent3
```

**On the server VM** (`multipass shell k3s-server`):

```bash
curl -sfL https://get.k3s.io | sh -

# the join token for the agents:
sudo cat /var/lib/rancher/k3s/server/node-token
# the server's IP:
ip -4 addr show
```

**On each agent VM:**

```bash
curl -sfL https://get.k3s.io | K3S_URL=https://<SERVER_IP>:6443 \
  K3S_TOKEN=<NODE_TOKEN> sh -
```

**Back on your machine** — take the kubeconfig from the server so `kubectl`
works from your desk (same procedure for the Pis later):

```bash
multipass exec k3s-server -- sudo cat /etc/rancher/k3s/k3s.yaml > ~/.kube/quiz-cluster.yaml
# replace 127.0.0.1 with the server IP inside that file, then:
export KUBECONFIG=~/.kube/quiz-cluster.yaml
kubectl get nodes
```

Deployment is the same `kubectl apply -k k8s/` — but here the images must come
from a registry (Docker Hub via CI), since there's no `k3d image import`.

## 4. The real thing — 4× Raspberry Pi 5

### Prepare each Pi

1. Flash **Raspberry Pi OS Lite (64-bit)** or **Ubuntu Server 24.04 arm64**
   (use the Pi Imager, preconfigure SSH + hostname: `pi-server`, `pi-agent1…3`).
2. **Static IPs** — either in your router (DHCP reservation, easiest) or on the
   Pi. The agents reference the server by IP; it must not change.
3. **Enable cgroups** (k3s won't start without this on Raspberry Pi OS).
   Append to the single line in `/boot/firmware/cmdline.txt`:

   ```
   cgroup_memory=1 cgroup_enable=memory
   ```

   then reboot. (Ubuntu Server doesn't need this.)
4. Recommended: disable swap, and update: `sudo apt update && sudo apt full-upgrade`.

### Install

Identical to Option B — server on `pi-server`:

```bash
curl -sfL https://get.k3s.io | sh -
sudo cat /var/lib/rancher/k3s/server/node-token
```

Agents on `pi-agent1…3`:

```bash
curl -sfL https://get.k3s.io | K3S_URL=https://<PI_SERVER_IP>:6443 \
  K3S_TOKEN=<NODE_TOKEN> sh -
```

Copy `/etc/rancher/k3s/k3s.yaml` from `pi-server` to your machine as in
Option B (replace `127.0.0.1` with the Pi's IP). Verify: `kubectl get nodes`
shows all 4, the server is schedulable (it's a worker too).

### Deploy the app

```bash
# real secrets instead of the dev placeholders in k8s/secrets.yaml:
kubectl create namespace quiz-app
kubectl -n quiz-app create secret generic quiz-app-secrets \
  --from-literal=JWT_SECRET="$(openssl rand -hex 32)" \
  --from-literal=POSTGRES_USER=admin \
  --from-literal=POSTGRES_PASSWORD="$(openssl rand -hex 16)"

# set the Docker Hub account in k8s/kustomization.yaml (images: newName),
# remove secrets.yaml from the resources list for prod, then:
kubectl apply -k k8s/
```

The CI pipeline builds `linux/arm64` images on `v*` tags, so the Pis pull and
run them natively.

### Expose via ngrok

Traefik listens on port 80 of **every** node. Run ngrok on any Pi (or any
machine that reaches one):

```bash
ngrok http 80
```

Single tunnel, single origin — frontend, REST and WebSockets all go through it.
Because the frontend uses relative `/api/...` URLs and derives `wss://` from
`location`, no rebuild is needed for the ngrok domain.

## 5. Troubleshooting quick reference

| Symptom | First command | Usual cause |
|---|---|---|
| Pod `Pending` | `kubectl -n quiz-app describe pod <pod>` | no node has resources / PVC unbound |
| Pod `ImagePullBackOff` | same | image name/tag wrong, not pushed, or `:latest` with locally-imported image |
| Pod `CrashLoopBackOff` | `kubectl -n quiz-app logs <pod> --previous` | missing env var (the Rust services `expect()` them), DB not ready yet |
| 404 from Traefik | `kubectl -n quiz-app get ingress,middleware` | middleware annotation/namespace prefix mismatch |
| Service unreachable | `kubectl -n quiz-app get endpoints` | label selector doesn't match pod labels, or readiness probe failing |
| Postgres DBs missing | `kubectl -n quiz-app exec -it deploy/postgres -- psql -U admin -l` | init script only runs on a **fresh** volume — delete PVC to re-run |

Worth installing: `k9s` (terminal cluster UI, `sudo pacman -S k9s`).
