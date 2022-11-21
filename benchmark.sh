#!/bin/bash
address=$1
port=$(kubectl get svc new-cache-chain-node0-nodeport -ncita |grep '8000' | awk '{print $5}')
port=${port: -9}
port=${port: 0:5}
sed  -i "" 's/^  "to": \(.*\)/  "to": "'$address'",/' /Users/naughtydog/Desktop/benchmark/data.json
echo "ab -n 8192 -c 1000 -T application/json -p /Users/naughtydog/Desktop/benchmark/data.json http://192.168.10.120:$port/api/sendTx"
ab -n 8192 -c 1000 -T application/json -p /Users/naughtydog/Desktop/benchmark/data.json http://192.168.10.120:$port/api/sendTx