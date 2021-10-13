# Copyright (c) Facebook, Inc. and its affiliates.
# SPDX-License-Identifier: Apache-2.0

import sys
import os
import csv
import matplotlib.pyplot as plt
import argparse


def read_csv(csv_path):
    with open(csv_path) as csv_file:
        data = list(csv.reader(csv_file))
    return data


def plot_data(csv_data):
    node_num = len(csv_data[0])
    max_clock = int(max(csv_data[-1]))

    node_switches = []
    for node in range(node_num):
        node_range = []
        curr_round = 0
        prev_clock = 0
        for i in range(len(csv_data)):
            clock_switch = csv_data[i][node]
            if clock_switch:
                clock_switch = int(clock_switch) # clock_switch is not an empty string, therefore can be cast safelyr
                node_range += [curr_round]*(clock_switch-prev_clock)
                curr_round = i
                prev_clock = int(clock_switch)
        node_range += [curr_round] * (100 + (max_clock - prev_clock)) # make sure all the elements are the same length
        node_switches.append(node_range)

    plt.figure()
    for node in node_switches:
        plt.plot(range(len(node)), node)

    plt.legend(["Node: " + str(x) for x in range(node_num)])
    plt.xlabel('Time')
    plt.ylabel('Round number')
    plt.grid(axis='both', which='both')
    plt.show()


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("csv_path", help="Path of the round switch csv file created by the consensus simulator")
    args = parser.parse_args()
    if os.path.exists(args.csv_path):
        csv_data = read_csv(args.csv_path)
        csv_data = csv_data[1::]
    else:
        sys.exit("Provide a path of the csv file for the round switches")
    plot_data(csv_data)
