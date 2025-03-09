from typing import Tuple
from scipy.signal import savgol_filter
import math

from matplotlib import pyplot as plt
import numpy as np
from scipy.signal import argrelextrema

data = np.load('../data_recording/DATA_8mhz.npy', 'r', allow_pickle=True, )[0]
# each sample is 250us ( 4 every us)
sampling_freq = 4e6
samples_per_symbol = 4
data_seconds = np.array_split(data,
                              5)  # split array into 1 second chunks (20,000,000 samples at 4MHz is 5 seconds) (4,000,000 samples each)
data_ms = np.array_split(data_seconds[0], 1000)  # split 1 second into ms chunks (4,000 samples each)


def smooth_data(data: np.ndarray) -> np.ndarray:
    data = savgol_filter(data, 50, 5)
    data = savgol_filter(data, 100, 5)
    data = savgol_filter(data, 100, 5)
    return data


def get_cut_off(data, slice_thickness=1e-3) -> Tuple[Tuple[float, float], Tuple[float, float]]:
    # get
    d = data
    # filter for points that have no imaginary component
    points = np.array([point for point in d if np.abs(point.imag) < slice_thickness])
    # get density of points along the real axis
    (hist, bin_edges) = np.histogram(np.real(points), bins="auto")

    # fill in missing points with avg
    def get_middle(arr, i) -> int:
        if arr[i] != 0:
            return arr[i]

        if (i + 1) > len(arr):
            return arr[i-1]

        if (i - 1) < 0:
            return arr[i+1]

        return (arr[i-1] + arr[i+1]) // 2

    hist = np.array([ get_middle(hist, i) for (i, x) in enumerate(hist)])


    left_half = hist[0:len(hist) // 2]
    # this double filter seems to work to smooth points
    left_half = smooth_data(left_half)
    # find local minimums, take the right most one
    mins = argrelextrema(left_half, np.less)[0]
    cutoff_left_inner = mins[-1]
    cutoff_left_outer= mins[-2]

    right_half = np.flip(hist[len(hist) // 2:])
    # this double filter seems to work to smooth points
    right_half = smooth_data(right_half)
    # find local minimums, take the right most one
    mins = argrelextrema(right_half, np.less)[0]
    cutoff_right_inner = len(hist) - mins[-1]
    cutoff_right_outer= len(hist) - mins[-2]
    x = bin_edges[:-1]
    return (x[cutoff_left_inner], x[cutoff_left_outer],), (x[cutoff_right_inner], x[cutoff_right_outer])



def lower_filter(data):
    rotation_list = []
    d = data

    def get_rotation_factor(degrees:int):
        theta = np.radians(degrees)
        return np.cos(theta) + 1j * np.sin(theta)

    n_splits = 4
    avg_sum = 0
    for i in range(0, n_splits):
        degrees =  i * (180/n_splits)
        print("deg",degrees)
        rotation_factor = get_rotation_factor(degrees)
        rotated_array = d * rotation_factor
        ((left_inner,_left_outer), (right_inner,_right_outer)) = get_cut_off(rotated_array)
        left_inner = (left_inner + 0j) * get_rotation_factor(degrees)
        right_inner = (right_inner + 0j) * get_rotation_factor(degrees)
        rotation_list.append(left_inner)
        rotation_list.append(right_inner)
    inner_cut =  np.mean(np.abs(rotation_list))
    return inner_cut





packet_sample_size = 46 * 16 * 4
break_size = 4000

data = data_seconds[0]

inner =  lower_filter(data)
inner = inner*0.6

test =  np.array([(x if abs(x) > inner else 0) for x in data])
print(test)


all_values = np.abs(test)

fudge = 0


def test_find():
    packet_size=(46 * 16 * 4) - fudge
    count = 0
    max_count = 0

    for i in range(0, len(all_values)):
        if all_values[i] > 0:
            count += 1
        else:
            max_count = max(count, max_count)
            count = 0
        if count >= packet_size:
            return i
    return -1, max_count, packet_size


if __name__ == '__main__':
    index = test_find()
    print("start", index)
    print("ns", index/4)
    print("ms", index/4000)












