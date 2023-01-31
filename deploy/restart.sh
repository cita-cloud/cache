#!/bin/bash
current_dir=$PWD
opt=$1
delete_image=$2
if [[ -z $opt ]]; then
  opt=a
fi
if [ $opt = a ]; then
  docker rm -f $(docker ps | grep master | awk '{print $1}')
  docker rm -f $(docker ps | grep validator | awk '{print $1}')
  rm -rf $current_dir/deploy/master/executor/data
  rm -rf $current_dir/deploy/validator/executor/data
  rm -rf $current_dir/deploy/master/executor/logs
  rm -rf $current_dir/deploy/validator/executor/logs
  rm -rf $current_dir/deploy/master/redis/volume
  rm -rf $current_dir/deploy/validator/redis/volume
  ls  $current_dir/deploy/master/executor
  ls  $current_dir/deploy/validator/executor
  ls  $current_dir/deploy/master/redis
  if [ $delete_image = d ]; then
    docker rmi -f $(docker images | grep cache | awk '{print $3}')
  fi
  /usr/local/bin/docker-compose -f $current_dir/deploy/master/docker-compose.yaml up -d
  /usr/local/bin/docker-compose -f $current_dir/deploy/validator/docker-compose.yaml up -d
elif [ $opt = v ]; then
  docker rm -f $(docker ps | grep validator | awk '{print $1}')
  rm -rf $current_dir/deploy/validator/executor/data
  rm -rf $current_dir/deploy/validator/executor/logs
  rm -rf $current_dir/deploy/validator/redis/volume
  if [ $delete_image = d ]; then
    docker rmi -f $(docker images | grep cache | awk '{print $3}')
  fi
  /usr/local/bin/docker-compose -f $current_dir/deploy/validator/docker-compose.yaml up -d
elif [ $opt = m ]; then
  docker rm -f $(docker ps | grep master | awk '{print $1}')
  rm -rf $current_dir/deploy/master/executor/data
  rm -rf $current_dir/deploy/master/executor/logs
  rm -rf $current_dir/deploy/master/redis/volume
  /usr/local/bin/docker-compose -f $current_dir/deploy/master/docker-compose.yaml up -d
  if [ $delete_image = d ]; then
    docker rmi -f $(docker images | grep cache | awk '{print $3}')
  fi
fi
