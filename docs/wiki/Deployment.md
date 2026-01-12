# Deployment Guide

Guía completa para desplegar Vortex Config en producción.

## Requirements

**System Dependencies:**
- Git 2.x+ installed on the system
- Docker (for containerized deployment) or Rust 1.92+ (for source deployment)

**Note:** Vortex Config uses the system's `git` CLI for repository operations, ensuring maximum compatibility. The Docker image includes git by default.

---

## Docker Deployment

### Quick Start

```bash
docker run -d \
  --name vortex-config \
  -p 8888:8888 \
  -e GIT_URI=https://github.com/your-org/config-repo.git \
  -e GIT_USERNAME=${GIT_USERNAME} \
  -e GIT_PASSWORD=${GIT_TOKEN} \
  -v vortex-repos:/var/lib/vortex/repos \
  --restart unless-stopped \
  vortex-config:latest
```

### Production Configuration

```bash
docker run -d \
  --name vortex-config \
  -p 8888:8888 \
  -e VORTEX_HOST=0.0.0.0 \
  -e VORTEX_PORT=8888 \
  -e GIT_URI=https://github.com/your-org/config-repo.git \
  -e GIT_DEFAULT_LABEL=main \
  -e GIT_USERNAME=${GIT_USERNAME} \
  -e GIT_PASSWORD=${GIT_TOKEN} \
  -e GIT_REFRESH_INTERVAL_SECS=60 \
  -e VORTEX_CACHE_TTL_SECONDS=600 \
  -e VORTEX_CACHE_MAX_CAPACITY=50000 \
  -e RUST_LOG=warn \
  -v vortex-repos:/var/lib/vortex/repos \
  --memory="256m" \
  --cpus="0.5" \
  --restart always \
  --health-cmd="curl -f http://localhost:8888/health || exit 1" \
  --health-interval=30s \
  --health-timeout=10s \
  --health-retries=3 \
  vortex-config:latest
```

---

## Docker Compose

### docker-compose.yml

```yaml
version: '3.8'

services:
  vortex-config:
    image: vortex-config:latest
    container_name: vortex-config
    ports:
      - "8888:8888"
    environment:
      - VORTEX_HOST=0.0.0.0
      - VORTEX_PORT=8888
      - GIT_URI=${GIT_URI}
      - GIT_DEFAULT_LABEL=main
      - GIT_USERNAME=${GIT_USERNAME}
      - GIT_PASSWORD=${GIT_PASSWORD}
      - GIT_REFRESH_INTERVAL_SECS=60
      - VORTEX_CACHE_TTL_SECONDS=600
      - VORTEX_CACHE_MAX_CAPACITY=50000
      - RUST_LOG=info
    volumes:
      - vortex-repos:/var/lib/vortex/repos
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8888/health"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 10s
    deploy:
      resources:
        limits:
          cpus: '0.5'
          memory: 256M
        reservations:
          cpus: '0.1'
          memory: 64M

volumes:
  vortex-repos:
    driver: local
```

### .env

```bash
GIT_URI=https://github.com/your-org/config-repo.git
GIT_USERNAME=your-username
GIT_PASSWORD=ghp_yourtoken123
```

### Run

```bash
docker-compose up -d
docker-compose logs -f
```

---

## Kubernetes Deployment

### Namespace

```yaml
apiVersion: v1
kind: Namespace
metadata:
  name: vortex-config
```

### ConfigMap

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: vortex-config
  namespace: vortex-config
data:
  VORTEX_HOST: "0.0.0.0"
  VORTEX_PORT: "8888"
  GIT_URI: "https://github.com/your-org/config-repo.git"
  GIT_DEFAULT_LABEL: "main"
  GIT_REFRESH_INTERVAL_SECS: "60"
  VORTEX_CACHE_TTL_SECONDS: "600"
  VORTEX_CACHE_MAX_CAPACITY: "50000"
  RUST_LOG: "warn"
```

### Secret

```yaml
apiVersion: v1
kind: Secret
metadata:
  name: vortex-git-credentials
  namespace: vortex-config
type: Opaque
stringData:
  GIT_USERNAME: your-username
  GIT_PASSWORD: ghp_yourtoken123
```

### PersistentVolumeClaim

```yaml
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: vortex-repos-pvc
  namespace: vortex-config
spec:
  accessModes:
    - ReadWriteOnce
  resources:
    requests:
      storage: 5Gi
  storageClassName: standard
```

### Deployment

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: vortex-config
  namespace: vortex-config
  labels:
    app: vortex-config
spec:
  replicas: 2
  strategy:
    type: RollingUpdate
    rollingUpdate:
      maxSurge: 1
      maxUnavailable: 0
  selector:
    matchLabels:
      app: vortex-config
  template:
    metadata:
      labels:
        app: vortex-config
      annotations:
        prometheus.io/scrape: "true"
        prometheus.io/port: "8888"
        prometheus.io/path: "/metrics"
    spec:
      containers:
      - name: vortex-config
        image: vortex-config:0.5.0
        imagePullPolicy: IfNotPresent
        ports:
        - name: http
          containerPort: 8888
          protocol: TCP
        envFrom:
        - configMapRef:
            name: vortex-config
        env:
        - name: GIT_USERNAME
          valueFrom:
            secretKeyRef:
              name: vortex-git-credentials
              key: GIT_USERNAME
        - name: GIT_PASSWORD
          valueFrom:
            secretKeyRef:
              name: vortex-git-credentials
              key: GIT_PASSWORD
        volumeMounts:
        - name: repos
          mountPath: /var/lib/vortex/repos
        resources:
          requests:
            memory: "64Mi"
            cpu: "100m"
          limits:
            memory: "256Mi"
            cpu: "500m"
        livenessProbe:
          httpGet:
            path: /health
            port: 8888
          initialDelaySeconds: 10
          periodSeconds: 10
          timeoutSeconds: 5
          failureThreshold: 3
        readinessProbe:
          httpGet:
            path: /health
            port: 8888
          initialDelaySeconds: 5
          periodSeconds: 5
          timeoutSeconds: 3
          failureThreshold: 2
        securityContext:
          runAsNonRoot: true
          runAsUser: 1000
          allowPrivilegeEscalation: false
          readOnlyRootFilesystem: false
      volumes:
      - name: repos
        persistentVolumeClaim:
          claimName: vortex-repos-pvc
```

### Service

```yaml
apiVersion: v1
kind: Service
metadata:
  name: vortex-config
  namespace: vortex-config
  labels:
    app: vortex-config
spec:
  type: ClusterIP
  ports:
  - port: 8888
    targetPort: 8888
    protocol: TCP
    name: http
  selector:
    app: vortex-config
```

### Ingress (Optional)

```yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: vortex-config
  namespace: vortex-config
  annotations:
    nginx.ingress.kubernetes.io/rewrite-target: /
    cert-manager.io/cluster-issuer: "letsencrypt-prod"
spec:
  ingressClassName: nginx
  tls:
  - hosts:
    - config.yourdomain.com
    secretName: vortex-config-tls
  rules:
  - host: config.yourdomain.com
    http:
      paths:
      - path: /
        pathType: Prefix
        backend:
          service:
            name: vortex-config
            port:
              number: 8888
```

### Deploy

```bash
kubectl apply -f namespace.yaml
kubectl apply -f configmap.yaml
kubectl apply -f secret.yaml
kubectl apply -f pvc.yaml
kubectl apply -f deployment.yaml
kubectl apply -f service.yaml
kubectl apply -f ingress.yaml

# Verify
kubectl get pods -n vortex-config
kubectl logs -f -n vortex-config deployment/vortex-config
```

---

## High Availability Setup

### Load Balancing

**Kubernetes:** Service ClusterIP con múltiples réplicas

**Nginx:**

```nginx
upstream vortex_backend {
    least_conn;
    server vortex-1:8888;
    server vortex-2:8888;
    server vortex-3:8888;
}

server {
    listen 80;
    server_name config.yourdomain.com;

    location / {
        proxy_pass http://vortex_backend;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    }
}
```

### Shared Storage

**Problema:** Múltiples réplicas clonando el mismo repositorio

**Solución:**
- **PersistentVolume ReadWriteMany:** NFS, EFS (AWS), Filestore (GCP)
- **Sidecar pattern:** Init container clona repo, múltiples pods comparten volume

### Cache Consistency

**Actualmente:** Cache local por pod (no compartido)

**Future (Epic 8):** Distributed cache con Redis/Valkey

---

## Monitoring & Observability

### Prometheus

**ServiceMonitor (Prometheus Operator):**

```yaml
apiVersion: monitoring.coreos.com/v1
kind: ServiceMonitor
metadata:
  name: vortex-config
  namespace: vortex-config
spec:
  selector:
    matchLabels:
      app: vortex-config
  endpoints:
  - port: http
    path: /metrics
    interval: 30s
```

### Grafana Dashboard

**Métricas clave:**
- `vortex_cache_hits_total`
- `vortex_cache_misses_total`
- `vortex_http_requests_total`
- `vortex_http_request_duration_seconds`

### Logging

**Fluentd/Fluent Bit:**

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: fluent-bit-config
data:
  filter.conf: |
    [FILTER]
        Name parser
        Match *
        Key_Name log
        Parser json
```

**Loki:**

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: promtail-config
data:
  promtail.yaml: |
    clients:
      - url: http://loki:3100/loki/api/v1/push
```

---

## Security Hardening

### Network Policies

```yaml
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: vortex-config-network-policy
  namespace: vortex-config
spec:
  podSelector:
    matchLabels:
      app: vortex-config
  policyTypes:
  - Ingress
  - Egress
  ingress:
  - from:
    - namespaceSelector:
        matchLabels:
          name: default
    ports:
    - protocol: TCP
      port: 8888
  egress:
  - to:
    - namespaceSelector: {}
    ports:
    - protocol: TCP
      port: 443  # HTTPS Git
```

### Pod Security Policy

```yaml
apiVersion: policy/v1beta1
kind: PodSecurityPolicy
metadata:
  name: vortex-config-psp
spec:
  privileged: false
  allowPrivilegeEscalation: false
  runAsUser:
    rule: MustRunAsNonRoot
  seLinux:
    rule: RunAsAny
  fsGroup:
    rule: RunAsAny
  volumes:
  - 'configMap'
  - 'secret'
  - 'persistentVolumeClaim'
```

### Service Mesh (Istio)

**mTLS entre servicios:**

```yaml
apiVersion: security.istio.io/v1beta1
kind: PeerAuthentication
metadata:
  name: default
  namespace: vortex-config
spec:
  mtls:
    mode: STRICT
```

---

## Backup & Disaster Recovery

### Git Repository

**Backup:** El repositorio Git ES el backup (truth source)

**Disaster Recovery:**
- Clonar repo en múltiples zonas/regiones
- Mirror en GitLab/Bitbucket

### Cache

**No es crítico:** Se reconstruye desde Git

**Warm-up (optional):**

```bash
# Script para pre-calentar cache
for app in myapp payment order; do
  for env in dev staging prod; do
    curl http://vortex:8888/$app/$env
  done
done
```

---

## Scaling

### Horizontal Scaling

```bash
# Kubernetes
kubectl scale deployment vortex-config --replicas=5 -n vortex-config

# Docker Swarm
docker service scale vortex_config=5
```

### Vertical Scaling

```yaml
resources:
  requests:
    memory: "128Mi"
    cpu: "200m"
  limits:
    memory: "512Mi"
    cpu: "1000m"
```

### Auto-scaling (HPA)

```yaml
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: vortex-config-hpa
  namespace: vortex-config
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: vortex-config
  minReplicas: 2
  maxReplicas: 10
  metrics:
  - type: Resource
    resource:
      name: cpu
      target:
        type: Utilization
        averageUtilization: 70
  - type: Resource
    resource:
      name: memory
      target:
        type: Utilization
        averageUtilization: 80
```

---

## Troubleshooting

### Pods en CrashLoopBackOff

```bash
kubectl describe pod <pod-name> -n vortex-config
kubectl logs <pod-name> -n vortex-config --previous
```

**Causas comunes:**
- GIT_URI inválida
- Credenciales incorrectas
- Timeout de clone en repo grande

### Latencia alta

```bash
# Verificar métricas
kubectl port-forward -n vortex-config svc/vortex-config 8888:8888
curl http://localhost:8888/metrics | grep vortex_cache

# Aumentar cache
kubectl set env deployment/vortex-config VORTEX_CACHE_MAX_CAPACITY=100000 -n vortex-config
```

### Memoria alta

```bash
# Verificar uso de memoria
kubectl top pods -n vortex-config

# Reducir cache
kubectl set env deployment/vortex-config VORTEX_CACHE_MAX_CAPACITY=10000 -n vortex-config
```

---

## Production Checklist

- [ ] Git credentials en Secret (no en ConfigMap)
- [ ] PersistentVolume para `/var/lib/vortex/repos`
- [ ] Resource limits configurados
- [ ] Health checks configurados
- [ ] Logging centralizado (Loki/ELK)
- [ ] Métricas en Prometheus
- [ ] Grafana dashboard
- [ ] Network policies aplicadas
- [ ] TLS en ingress
- [ ] Backup del repositorio Git
- [ ] Runbook de incident response

---

## Próximos Pasos

- **[Configuration](Configuration.md)** - Opciones de configuración
- **[API Reference](API-Reference.md)** - API completa
- **[Architecture](Architecture.md)** - Arquitectura interna
