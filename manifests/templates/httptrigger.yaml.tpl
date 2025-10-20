apiVersion: control.cosmonic.io/v1alpha1
kind: HTTPTrigger
metadata:
  name: {{ app_name }}
  namespace: {{ namespace }}
  labels:
    app: {{ app_name }}
    version: {{ version }}
spec:
  deployPolicy: RollingUpdate
  ingress:
    host: '*'
    paths:
    - path: /
      pathType: Prefix
  replicas: 2
  template:
    spec:
      components:
      - image: {{ image }}
        name: {{ app_name }}
