#!/usr/bin/env python3

# This scripts outputs TPS stats in runtime
# Run example: ./tx_stats.py node.hostname.com:8080
# Also possible to dump statistic into cvs files if you provide
# path to file
# E.g. ./tx_stats.py node.hostname.com:8080 /path/to/stat.cvs

import csv
import requests
import sys
from datetime import datetime
from time import sleep
from prometheus_client import CollectorRegistry, Gauge, push_to_gateway
import argparse
from urllib.parse import urlparse

count_blocks = 10

def get_hostname(h):
    if "http" in h:
        return h
    else:
        return "http://" + h


def parse_datetime(d_time):
    d_time_parts = d_time[:-1].split(".")
    return datetime.strptime(
        d_time_parts[0] + "." + d_time_parts[1][:5], "%Y-%m-%dT%H:%M:%S.%f"
    )


def update_stats(stats, data):
    times = data["times"]
    for i, block in enumerate(data["blocks"]):
        height = block["height"]
        tx_count = block["tx_count"]
        if height in stats:  # skip existence entries
            continue
        if tx_count == 0:  # skip blocks with no transactions
            continue
        if (
            i == len(data["blocks"]) - 1
        ):  # skip last block info, we won't be able to calculate time delta
            continue
        block_time = (
            parse_datetime(times[i]) - parse_datetime(times[i + 1])
        ).total_seconds()
        stats[height] = tx_count / block_time


def calc_min_tps(stats):
    try:
        return int(min(stats.values()))
    except ValueError:
        return 0


def calc_max_tps(stats):
    try:
        return int(max(stats.values()))
    except ValueError:
        return 0


def calc_average_tps(stats):
    count = len(stats)
    if count == 0:
        return 0
    return int(sum(stats.values()) / count)


def calc_current_tps(data):
    times = data["times"]
    delta_time = parse_datetime(times[0]) - parse_datetime(times[1])
    return int(data["blocks"][0]["tx_count"] / delta_time.total_seconds())


def dump_statistic(file, stats):
    with open(file, "w") as f:
        w = csv.DictWriter(f, ["height", "TPS"])
        w.writeheader()
        for height in sorted(stats.keys()):
            w.writerow({"height": height, "TPS": stats[height]})


def main():
    parser = argparse.ArgumentParser(description='Exonum node\'s TPS stats collector')
    parser.add_argument('-s', '--service', action='store_true', help='Run as a system service and export metrics to Prometheus')
    parser.add_argument('-n', '--node', type=str, help='Exonum node\'s address', required=True)
    parser.add_argument('-p', '--pushgateway', nargs=1, type=str, help='Prometheus pushgateway address')
    parser.add_argument('-o', '--output', nargs=1, type=str, help='File name to dump data as CSV')

    args = parser.parse_args()

    hostname = get_hostname(args.node)

    blocks_url = "{}/api/explorer/v1/blocks?count={}&add_blocks_time=true".format(
        hostname, count_blocks
    )
    stats = dict()

    if (args.service and not args.pushgateway):
        print('Pushgateway address required in service mode')
        exit(1)

    if (args.pushgateway):
        registry = CollectorRegistry()
        grouping_keys = {}
        metric_current_tps_name = 'exonum_node_tps_current'
        metric_avg_tps_name = 'exonum_node_tps_average'
        metric_current_height_name = 'exonum_node_current_height'
        metric_avg_tps = Gauge(metric_avg_tps_name, 'Exonum\'s node average TPS', registry=registry)
        metric_current_height = Gauge(metric_current_height_name, 'Exonum\'s node current height', registry=registry)
        metric_current_tps = Gauge(metric_current_tps_name, 'Exonum\'s node current TPS', registry=registry)
        grouping_keys['instance'] = urlparse(hostname).netloc

    while True:
        try:
            response = requests.get(blocks_url)

            if response.status_code == 200:
                data = response.json()
                update_stats(stats, data)
                min_tps = calc_min_tps(stats)
                max_tps = calc_max_tps(stats)
                avrg_tps = calc_average_tps(stats)
                current_tps = calc_current_tps(data)
                last_height = int(data["range"]["end"])
                if (args.pushgateway):
                    try:
                        metric_avg_tps.set(avrg_tps)
                        metric_current_tps.set(current_tps)
                        metric_current_height.set(last_height)
                        push_to_gateway(args.pushgateway[0], job='StressTesting', registry=registry, grouping_key=grouping_keys)
                    except Exception as e:
                        print('Cannot send to prometheus: {}'.format(e))
                if (not args.service):
                    print(
                        "min: {}, max: {}, avrg: {}, current: {}, last height: {}".format(
                            min_tps, max_tps, avrg_tps, current_tps, last_height
                        ),
                        end="\r",
                    )
            else:
                print("Bad request", end="\r")

        except requests.exceptions.ConnectionError:
            print("Couldn't connect to host, Trying once again...", end="\r")

            sleep(1)

        except KeyboardInterrupt:
            print("Exit...")
            break

    if (args.output):
        dump_statistic(args.output[0], stats)


if __name__ == "__main__":
    main()
