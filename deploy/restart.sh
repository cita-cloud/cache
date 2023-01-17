#!/bin/bash
docker rm -f $(docker ps | grep master | awk '{print $1}')
docker rm -f $(docker ps | grep validator | awk '{print $1}')
rm -rf /Users/naughtydog/CLionProjects/cache/deploy/master/executor/data
rm -rf /Users/naughtydog/CLionProjects/cache/deploy/validator/executor/data
rm -rf /Users/naughtydog/CLionProjects/cache/deploy/master/executor/logs
rm -rf /Users/naughtydog/CLionProjects/cache/deploy/validator/executor/logs
rm -rf /Users/naughtydog/CLionProjects/cache/deploy/master/redis/volume
rm -rf /Users/naughtydog/CLionProjects/cache/deploy/validator/redis/volume
#docker rmi -f $(docker images | grep cache | awk '{print $3}')
/usr/local/bin/docker-compose up -d