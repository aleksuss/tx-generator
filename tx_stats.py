#!/usr/bin/env python3

# This scripts outputs TPS stats in runtime
# Run example: ./tx_stats.py node.hostname.com:8080

import requests
import sys
from datetime import datetime
from time import sleep

count_blocks = 10


def get_hostname():
    if len(sys.argv) < 2:
        print("Provide a hostname of the node")
        exit(1)
    if "http" in sys.argv[1]:
        return sys.argv[1]
    else:
        return "http://" + sys.argv[1]


def parse_datetime(d_time):
    return datetime.strptime(d_time[:-4], "%Y-%m-%dT%H:%M:%S.%f")


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


def main():
    hostname = get_hostname()
    blocks_url = "{}/api/explorer/v1/blocks?count={}&add_blocks_time=true".format(
        hostname, count_blocks
    )
    stats = dict()
    print("TPS statistics for host: %s" % hostname)

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
                print(
                    "min: {}, max: {}, avrg: {}, current: {}, last height: {}".format(
                        min_tps, max_tps, avrg_tps, current_tps, last_height
                    ),
                    end="\r",
                )
            else:
                print("Bad request")

            sleep(1)

        except KeyboardInterrupt:
            print("Exit...")
            exit(0)


if __name__ == "__main__":
    main()
