#!/bin/sh
case "$DSH_TEST_MODE" in
  ready)
    printf 'dsh web: http://127.0.0.1:43123/\n'
    while true; do sleep 1; done
    ;;
  exit)
    printf 'dsh web: http://127.0.0.1:43123/\n'
    exit 17
    ;;
  timeout)
    sleep 30
    ;;
esac
