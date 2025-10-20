apiVersion: apps/v1
kind: Deployment
metadata:
  name: {{ app_name }}
  namespace: {{ namespace }}
  labels:
    app: {{ app_name }}
spec:
  replicas: 2
  selector:
    matchLabels:
      app: {{ app_name }}
  template:
    metadata:
      labels:
        app: {{ app_name }}
    spec:
      imagePullSecrets:
        - name: ghcr-secret
      containers:
      - name: mcp-server
        image: {{ image }}
        ports:
        - containerPort: 8080
          name: http
        env:
        - name: LOG_LEVEL
          value: "info"
        resources:
          limits:
            memory: "512Mi"
            cpu: "1000m"
          requests:
            memory: "128Mi"
            cpu: "100m"
        livenessProbe:
          httpGet:
            path: /health
            port: 8080
          initialDelaySeconds: 10
          periodSeconds: 30
        readinessProbe:
          httpGet:
            path: /health
            port: 8080
          initialDelaySeconds: 5
          periodSeconds: 10
---
apiVersion: v1
kind: Service
metadata:
  name: {{ app_name }}
  namespace: {{ namespace }}
  labels:
    app: {{ app_name }}
spec:
  type: ClusterIP
  selector:
    app: {{ app_name }}
  ports:
  - port: 80
    targetPort: 8080
    protocol: TCP
    name: http
---
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: {{ app_name }}
  namespace: {{ namespace }}
  annotations:
    nginx.ingress.kubernetes.io/rewrite-target: /
spec:
  ingressClassName: nginx
  rules:
  - http:
      paths:
      - path: /mcp
        pathType: Prefix
        backend:
          service:
            name: {{ app_name }}
            port:
              number: 80
