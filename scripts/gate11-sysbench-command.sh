#!/bin/sh
set -eu

test "$#" -eq 3 || { printf '%s\n' 'usage: gate11-sysbench-command WORKLOAD THREADS DURATION' >&2; exit 2; }
workload=$1
threads=$2
duration=$3

case "$workload" in
    cpu)
        exec /usr/bin/sysbench cpu --threads="$threads" --time="$duration" \
            --cpu-max-prime=10000 run
        ;;
    memory)
        exec /usr/bin/sysbench memory --threads="$threads" --time="$duration" \
            run
        ;;
    threads)
        exec /usr/bin/sysbench threads --threads="$threads" --time="$duration" \
            --thread-yields=1000 --thread-locks=8 run
        ;;
    mutex)
        exec /usr/bin/sysbench mutex --threads="$threads" run
        ;;
    *) printf 'invalid workload: %s\n' "$workload" >&2; exit 2 ;;
esac
