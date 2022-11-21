#!/bin/bash
kubectl delete -f /Users/naughtydog/CLionProjects/cache/deploy.yml -ncita
kubectl delete pvc datadir-new-cache-chain-node0-0 -ncita
kubectl apply -f /Users/naughtydog/CLionProjects/cache/deploy.yml -ncita
port=$(kubectl get svc new-cache-chain-node0-nodeport -ncita |grep '8000' | awk '{print $5}')
port=${port: -9}
port=${port: 0:5}
echo $port