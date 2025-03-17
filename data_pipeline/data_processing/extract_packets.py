from ipywidgets import widgets
from math import floor
from numpy import floating
from numpy._typing import _32Bit
from prometheus_client.decorator import append
from random import random
from typing import Tuple, override
from scipy.signal import savgol_filter
from scipy import signal
from scipy.spatial.distance import cosine
import math

from matplotlib import pyplot as plt
from matplotlib.animation import FuncAnimation

import numpy as np
from numpy.typing import NDArray

from scipy.signal import argrelextrema
from bokeh.plotting import figure, show, output_notebook
from bokeh.models import Span

from typing import Annotated, Literal

from annotated_types import Gt

from pydantic import BaseModel
import pydantic_numpy.typing as pnd
from pydantic_numpy import np_array_pydantic_annotated_typing
from pydantic_numpy.model import NumpyModel, MultiArrayNumpyFile

from typing import List, Tuple, Dict


def plot_eye_cmp(s1, s2, samples_per_symbol, s1_title='', s2_title='', sup_title='', compensate_oqpsk=True,
                 disable_combo=False,
                 combo_title: None | str = None, combo_opacity=(100, 100)):
    if combo_title is None:
        combo_title = f"{s1_title}/{s2_title}"

    color1 = "#58A4B0"  # blue
    color2 = "#F44E3F"  # red
    # if enable_time_gradient:
    #     generate_gradient

    s1I, s1Q = (np.real(s1), np.imag(s1))
    s2I, s2Q = (np.real(s2), np.imag(s2))
    # compensate_oqpsk=False
    if compensate_oqpsk:
        # compensate for O-QPSK by shifting Q-phase back by T_c
        s1I, s1Q = (s1I[0:-samples_per_symbol // 2], s1Q[samples_per_symbol // 2:])
        s2I, s2Q = (s2I[0:-samples_per_symbol // 2], s2Q[samples_per_symbol // 2:])

    window_size = 64

    def create_plot(first_plot_range: Tuple[int, int]):
        # print(first_plot_range)
        #
        # start_1, end_1 = first_plot_range

        fig, (ax1, ax2, bx1, bx2, ax_combo) = (None, (None, None, None, None, None))
        if disable_combo:
            # fig, ((ax1, ax2,), (bx1, bx2)) = plt.subplots(2, 2, layout="constrained", figsize=(10, 6))
            fig, (ax1, ax2,) = plt.subplots(1, 2, layout="constrained", figsize=(10, 6))
        else:
            fig, (ax1, ax2, ax_combo) = plt.subplots(1, 3, layout="constrained", figsize=(15, 6))

        ax1.set_title(s1_title)
        ax2.set_title(s2_title)
        if not disable_combo:
            ax_combo.set_title(combo_title)

        ax1.set_box_aspect(1)
        ax2.set_box_aspect(1)
        if not disable_combo:
            ax_combo.set_box_aspect(1)

        scatter_size = 5
        sc1 = ax1.scatter(s1I, s1Q, scatter_size, marker='o', c=np.linspace(0, 1, len(s1I)), cmap="spring", )
        plt.colorbar(sc1)  # Show color scale

        sc2 = ax2.scatter(s2I, s2Q, scatter_size, marker='o', c=np.linspace(0, 1, len(s2I)),
                          cmap="spring", )  # color=color2
        plt.colorbar(sc2)

        # bx1.scatter(s1I[start_1:end_1], s1Q[start_1:end_1], scatter_size, marker='o',  c=np.linspace(0, 1, end_1-start_1),
        #               cmap="spring",)  #color=color1
        # bx2.scatter(s2I[start_1:end_1], s2Q[start_1:end_1], scatter_size, marker='o', c=np.linspace(0, 1, end_1-start_1),
        #               cmap="spring")  #color=color1

        if not disable_combo:
            ax_combo.scatter(s1I, s1Q, scatter_size, marker='o', color=color1, alpha=combo_opacity[0] / 100)
            ax_combo.scatter(s2I, s2Q, scatter_size, marker='o', color=color2, alpha=combo_opacity[1] / 100)

        fig.suptitle(sup_title)
        plt.show()

    create_plot(None)
    # graph = widgets.interactive(create_plot,first_plot_range=widgets.SelectionRangeSlider(
    #     options=[i for i in range(0, len(s1I))],
    #     index=(0, 4),
    #     description='Range',
    #     disabled=False
    # ))
    # display(graph)


class Packet(BaseModel):
    data: np_array_pydantic_annotated_typing(dimensions=1, strict_data_typing=True, data_type=np.complex64)
    base_data: np_array_pydantic_annotated_typing(dimensions=1, strict_data_typing=True, data_type=np.complex64)
    start_index: int
    end_index: int

    @staticmethod
    def from_tuple(array, indexes: Tuple[int, int]) -> "Packet":
        start, end = indexes
        length = end - start
        padding = (4096 - length) / 2
        padding_start = int(padding)
        padding_end = int(padding)
        if padding % 1 != 0:
            padding_end += 1

        return Packet(data=array[(start):(end)], start_index=start, end_index=end, base_data=array)

    @staticmethod
    def from_tuple_with_avg_data_padding(array, indexes: Tuple[int, int]) -> "Packet":
        start, end = indexes
        diff = (46 * 16 * 4) - (end - start)
        return Packet(
            data=np.concatenate((array[start - diff:start], array[start:end], array[end + 1:end + diff + 1],)),
            start_index=start, end_index=end, base_data=array)

    def shift_start_back_by(self, i: int) -> np_array_pydantic_annotated_typing(dimensions=1, strict_data_typing=True,
                                                                                data_type=np.complex64):
        return self.base_data[self.start_index - i:self.end_index]


def min_max_scale(data):
    min_val = np.min(data)
    max_val = np.max(data)
    return (data - min_val) / (max_val - min_val)


def pipeline_normalize_to_magnitude(samples):
    return samples / np.mean(np.abs(samples))


def find_packets(data, num_samples_in_packet=46 * 16 * 4) -> List[Packet]:
    # scale magnitude between 0 and 1
    scaled_data = min_max_scale(np.abs(data))

    d_sav_10 = savgol_filter(scaled_data, 10, 1)

    d_sav_100 = savgol_filter(scaled_data, 100, 1)

    above_min_variance = 0.5

    avg_low = np.average(d_sav_100[d_sav_100 < min(d_sav_100) * (1 + above_min_variance)])

    square_wave_threshold = 0.5

    extracted_square_wave = np.where(d_sav_100 > (avg_low * (1 + square_wave_threshold)), 1, 0)

    # find the starts and ends of the detected data
    starts = np.where(np.concatenate((np.array(
        [x != extracted_square_wave[i + 1] and x == 0 for i, x in enumerate(extracted_square_wave[:-1])]), [False])))[0]
    ends = np.where(np.concatenate((np.array(
        [x != extracted_square_wave[i + 1] and x == 1 for i, x in enumerate(extracted_square_wave[:-1])]), [False])))[0]

    actual_packets = np.array([(start, end) for start, end in zip(starts, ends) if
                               num_samples_in_packet * 1.05 > (end - start) >= num_samples_in_packet])

    buffer = 4

    def get_adjusted_edges(edges):
        (start, end) = edges
        avg_high = np.average(d_sav_100[start:end])
        middle = (avg_high + avg_low) / 2
        above_middle = np.array([x for x in range(start, end) if d_sav_10[x] > middle])
        start, end = above_middle[0], above_middle[-1]
        # move start and end to the nearest micro_second boundary, (as long as the input data starts on a microsecond boundary this should work
        start -= (start % 4)
        end += (4 - (end % 4))
        # add a bit of buffer, needed to get correct packet values
        return start - buffer, (start - buffer) + 4096  # end

    packets = [Packet.from_tuple(data, tup) for tup in np.array([get_adjusted_edges(x) for x in actual_packets])]

    return packets


##################### from original pipeline ##############################
def matched_filter(samples, samples_per_symbol):
    n = np.arange(samples_per_symbol)
    pulse = np.sin(np.pi - np.pi * n / samples_per_symbol, dtype=np.float32)

    samples_filtered = np.convolve(samples, pulse, 'same') / (samples_per_symbol / 2)

    return samples_filtered


def sync_freq(samples, sampling_freq, fft_window_size, samples_per_symbol, debug=False):
    # triangle window
    # window = sp.signal.triang(N)

    # Gaussian window (r=8)
    t = np.arange(fft_window_size) - fft_window_size / 2
    window = np.exp(- 8 * 8 * t * t / (2 * fft_window_size * fft_window_size), dtype=np.float32)

    # coarse freq. recovery for OQPSK
    # see http://jontio.zapto.org/hda1/oqpsk.html
    samples_squared = window[:len(samples)] * samples[:len(window)] * samples[:len(window)]
    fft = np.abs(np.fft.fft(samples_squared, fft_window_size).astype(np.complex64))

    # find spectral peaks in both sides of spectrum,
    # in regions around the symbol rate (N/sps)
    bin_left = -fft_window_size // samples_per_symbol
    bin_right = fft_window_size // samples_per_symbol
    bin_range = 200  # ~200kHz on original frequency scale
    max_left_idx = np.argmax(fft[bin_left - bin_range:bin_left + bin_range]) + bin_left - bin_range
    max_right_idx = np.argmax(fft[bin_right - bin_range:bin_right + bin_range]) + bin_right - bin_range

    # parabolic interpolation
    # idx = max_left_idx
    # max_left_idx = idx + (fft[idx+1] - fft[idx-1])/(2*(2*fft[idx] - fft[idx+1] - fft[idx-1]))
    # idx = max_right_idx
    # max_right_idx = idx + (fft[idx+1] - fft[idx-1])/(2*(2*fft[idx] - fft[idx+1] - fft[idx-1]))

    # Gaussian interpolation
    idx = max_left_idx
    max_left_idx = idx + np.log(fft[idx + 1] / fft[idx - 1], dtype=np.float32) / (
            2 * np.log(fft[idx] * fft[idx] / (fft[idx + 1] * fft[idx - 1]), dtype=np.float32))
    idx = max_right_idx
    max_right_idx = idx + np.log(fft[idx + 1] / fft[idx - 1], dtype=np.float32) / (
            2 * np.log(fft[idx] * fft[idx] / (fft[idx + 1] * fft[idx - 1]), dtype=np.float32))

    freq_resolution = np.float32(sampling_freq / fft_window_size / 2)
    cfo:np.float32 = np.float32((max_left_idx + max_right_idx) / 2) * freq_resolution

    if debug:
        freq = np.fft.fftfreq(fft_window_size, 1 / sampling_freq) / 2
        plt.plot(freq, fft)
        plt.axvspan((bin_left - bin_range) * freq_resolution, (bin_left + bin_range) * freq_resolution, alpha=0.5,
                    color='g')
        plt.axvspan((bin_right - bin_range) * freq_resolution, (bin_right + bin_range) * freq_resolution, alpha=0.5,
                    color='g')
        plt.axvline(max_left_idx * freq_resolution, c='g', ls='--')
        plt.axvline(max_right_idx * freq_resolution, c='g', ls='--')
        plt.axvline(cfo, c='r', ls='--')
        plt.show()

    # correct frequency
    t = np.linspace(0, len(samples) / sampling_freq, len(samples), dtype=np.float32)

    # return corrected signal and CFO estimate
    return (samples * np.exp(-1j * 2 * np.pi * cfo * t, dtype=np.complex64), cfo)


def sync_phase2(samples, samples_per_symbol, resolution=16, cutoff=1500, threshold=0.35, debug=False):
    # todo: maybe implement binary search for enhanced efficiency?

    phases = np.arange(0, 1, 1 / resolution) * np.pi / 2
    outliers = np.empty(len(phases))
    for i, phase in enumerate(phases):
        samples_phase = samples[:cutoff] * np.exp(1j * phase)

        # todo: debug
        # plt.scatter(np.real(samples_phase)[0:-sps//2], np.imag(samples_phase)[sps//2:], 1, marker='.')
        # plt.gca().set_aspect(1)
        # plt.show()

        # compensate for OQPSK to get X-shape
        samples_phaseI, samples_phaseQ = np.real(samples_phase[0:-samples_per_symbol // 2]), np.imag(
            samples_phase[samples_per_symbol // 2:])

        # count all samples which are more than `threshold` away from X-shape as outliers
        outliers[i] = np.count_nonzero(np.abs(np.abs(samples_phaseI) - np.abs(samples_phaseQ)) > threshold)

    if debug:
        plt.plot(phases * 180 / np.pi, outliers, '.-')
        plt.xlabel('Phase Shift [°]')
        plt.ylabel('amount of samples outside X-shape')
        plt.show()

    phase_shift = phases[np.argmin(outliers)]
    samples_phase = samples * np.exp(1j * phase_shift)

    return samples_phase, phase_shift


ieee802154_chip_map = np.array([
    [1, 1, 0, 1, 1, 0, 0, 1, 1, 1, 0, 0, 0, 0, 1, 1, 0, 1, 0, 1, 0, 0, 1, 0, 0, 0, 1, 0, 1, 1, 1, 0],
    [1, 1, 1, 0, 1, 1, 0, 1, 1, 0, 0, 1, 1, 1, 0, 0, 0, 0, 1, 1, 0, 1, 0, 1, 0, 0, 1, 0, 0, 0, 1, 0],
    [0, 0, 1, 0, 1, 1, 1, 0, 1, 1, 0, 1, 1, 0, 0, 1, 1, 1, 0, 0, 0, 0, 1, 1, 0, 1, 0, 1, 0, 0, 1, 0],
    [0, 0, 1, 0, 0, 0, 1, 0, 1, 1, 1, 0, 1, 1, 0, 1, 1, 0, 0, 1, 1, 1, 0, 0, 0, 0, 1, 1, 0, 1, 0, 1],
    [0, 1, 0, 1, 0, 0, 1, 0, 0, 0, 1, 0, 1, 1, 1, 0, 1, 1, 0, 1, 1, 0, 0, 1, 1, 1, 0, 0, 0, 0, 1, 1],
    [0, 0, 1, 1, 0, 1, 0, 1, 0, 0, 1, 0, 0, 0, 1, 0, 1, 1, 1, 0, 1, 1, 0, 1, 1, 0, 0, 1, 1, 1, 0, 0],
    [1, 1, 0, 0, 0, 0, 1, 1, 0, 1, 0, 1, 0, 0, 1, 0, 0, 0, 1, 0, 1, 1, 1, 0, 1, 1, 0, 1, 1, 0, 0, 1],
    [1, 0, 0, 1, 1, 1, 0, 0, 0, 0, 1, 1, 0, 1, 0, 1, 0, 0, 1, 0, 0, 0, 1, 0, 1, 1, 1, 0, 1, 1, 0, 1],
    [1, 0, 0, 0, 1, 1, 0, 0, 1, 0, 0, 1, 0, 1, 1, 0, 0, 0, 0, 0, 0, 1, 1, 1, 0, 1, 1, 1, 1, 0, 1, 1],
    [1, 0, 1, 1, 1, 0, 0, 0, 1, 1, 0, 0, 1, 0, 0, 1, 0, 1, 1, 0, 0, 0, 0, 0, 0, 1, 1, 1, 0, 1, 1, 1],
    [0, 1, 1, 1, 1, 0, 1, 1, 1, 0, 0, 0, 1, 1, 0, 0, 1, 0, 0, 1, 0, 1, 1, 0, 0, 0, 0, 0, 0, 1, 1, 1],
    [0, 1, 1, 1, 0, 1, 1, 1, 1, 0, 1, 1, 1, 0, 0, 0, 1, 1, 0, 0, 1, 0, 0, 1, 0, 1, 1, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 1, 1, 1, 0, 1, 1, 1, 1, 0, 1, 1, 1, 0, 0, 0, 1, 1, 0, 0, 1, 0, 0, 1, 0, 1, 1, 0],
    [0, 1, 1, 0, 0, 0, 0, 0, 0, 1, 1, 1, 0, 1, 1, 1, 1, 0, 1, 1, 1, 0, 0, 0, 1, 1, 0, 0, 1, 0, 0, 1],
    [1, 0, 0, 1, 0, 1, 1, 0, 0, 0, 0, 0, 0, 1, 1, 1, 0, 1, 1, 1, 1, 0, 1, 1, 1, 0, 0, 0, 1, 1, 0, 0],
    [1, 1, 0, 0, 1, 0, 0, 1, 0, 1, 1, 0, 0, 0, 0, 0, 0, 1, 1, 1, 0, 1, 1, 1, 1, 0, 1, 1, 1, 0, 0, 0]
]) * 2 - 1


def sync_phase_time(samples, payload_first_2_bytes=(0, 0), chip_map=ieee802154_chip_map):
    # generate expected chip sequence for first payload byte as real array
    chips_expected = np.empty(32 * 2, dtype=int)
    d = int(payload_first_2_bytes[1])
    lsb = d & 0b1111
    msb = d >> 4
    chips_expected[0:32] = chip_map[lsb]
    chips_expected[32:64] = chip_map[msb]

    # roughly for two first payload bytes
    chips_sampled = np.empty(32 * 2 * 2, dtype=np.float32)

    # try four different possible phase offsets and get highest correlation
    (phase_offset_coarse, time_offset_coarse, max_corr) = (0, 0, 0)
    s = samples[:32 * 2]
    for i in range(4):
        if i % 2 == 0:
            chips_sampled[0::2] = np.real(s)
            chips_sampled[1::2] = np.imag(s)
        else:
            # switch I and Q to compensate for wrong IQ alignment during time synchronization
            chips_sampled[0::2] = np.imag(s)
            chips_sampled[1::2] = np.real(s)
        out = np.correlate(chips_sampled, chips_expected, 'full')

        # only consider beginning of samples
        corr = np.max(out[:128])
        if corr > max_corr:
            (max_i, time_offset_coarse, max_corr) = (i, np.argmax(out) - len(chips_expected), corr)
        s = s * np.exp(1j * np.pi / 2)
    # print("max corr:", max_corr)
    phase_offset_coarse = max_i * np.pi / 2

    samples_expected = chips_expected[0::2] + 1j * chips_expected[1::2]
    samples_corrected = samples * np.exp(1j * phase_offset_coarse)

    if max_i % 2 == 1:
        # roll Q by 1 to compensate for wrong IQ alignment during time synchronization
        samples_corrected[:-1] = np.real(samples_corrected[:-1]) + 1j * np.imag(samples_corrected[1:])

    return samples_corrected, samples_expected, -time_offset_coarse // 2, phase_offset_coarse


def sync_time(samples, samples_per_symbol, damping_factor=1, loop_bandwidth=0.01, gain_detector=2.7):
    # time synchronization for OQPSK
    # followed symbolSyncCodegen since symbolSync is natively implemented in C/C++ (Matlab MEX file)
    # see https://www.mathworks.com/help/comm/ref/comm.symbolsynchronizer-system-object.html#bumtxky-18

    # calculate constants
    K_p = gain_detector
    K_0 = -1  # Constant for modulo-1 counter
    theta = loop_bandwidth / samples_per_symbol / (damping_factor + 0.25 / damping_factor)
    d = (1 + 2 * damping_factor * theta + theta * theta) * K_0 * K_p
    gain_integrator = (4 * theta * theta) / d
    gain_proportional = (4 * damping_factor * theta) / d
    # Specify the maximum output frame length buffer's size for
    # numerical precision problem when using ceiling
    samples_corrected_max = int(np.ceil(len(samples) * 11 / (samples_per_symbol * 10)))
    alpha = 0.5
    filter_interpolation_coeff = np.array([
        [0, 0, 1, 0],  # Constant
        [-alpha, 1 + alpha, -(1 - alpha), -alpha],  # Linear
        [alpha, -alpha, -alpha, alpha]  # Quadratic
    ])

    # align symbols for OQPSK (assuming phase ambiguity 0/180°, otherwise needs to be correced afterwards)
    samples = np.real(samples[:-samples_per_symbol // 2]) + 1j * np.imag(samples[samples_per_symbol // 2:])

    # initialize variables
    timing_error_vec = np.zeros(len(samples), dtype=float)
    timing_error = 0
    samples_corrected = np.zeros(samples_corrected_max, dtype=np.complex64)
    strobe_count = 0
    strobe_flag = False
    strobe_history = np.zeros(samples_per_symbol)
    filter_interpolation_state = np.zeros(4, dtype=np.complex64)
    filter_interpolation_history = np.zeros(samples_per_symbol, dtype=np.complex64)
    filter_loop_state = 0
    filter_loop_prev = 0
    counter_val = 0

    # go over samples one by one
    for i in range(len(samples)):
        # todo: can we have bad receiver condition?
        timing_error_vec[i] = timing_error

        # Interpolation Filter
        # Piecewise parabolic interpolator in farrow structure with alpha = 0.5.
        # Refer to (8.72)-(8.73) on page 468 and Figure 8.4.16 on page 471 in Rice's book [1].
        filter_interpolation_state[0] = samples[i]
        filter_interpolation_out = filter_interpolation_coeff @ filter_interpolation_state @ np.array(
            [1, timing_error, timing_error])
        filter_interpolation_state[1:3] = filter_interpolation_state[0:2]

        if strobe_flag:
            samples_corrected[strobe_count] = filter_interpolation_out
            strobe_count += 1

        # Timing Error Detection (TED) with zero-crossing algorithm
        # TED calculation occurs on a strobe
        if strobe_flag and np.all(strobe_history[1:] == 0):
            # The above condition allows TED update after a skip. If we want
            # TED update to happen only at regular strobings, need to check
            # strobe_history[0] == 1 in addition to the condition above.

            sample_prev = filter_interpolation_history[0]
            sample_mid = filter_interpolation_history[len(filter_interpolation_history) // 2]
            e = (np.real(sample_mid) * (np.sign(np.real(sample_prev)) - np.sign(np.real(filter_interpolation_out))) +
                 np.imag(sample_mid) * (np.sign(np.imag(sample_prev)) - np.sign(np.imag(filter_interpolation_out))))
        else:
            e = 0

        # Stuffing and skipping (p. 490-494 in Rice's book)
        strobe_history = np.roll(strobe_history, -1)
        strobe_history[-1] = strobe_flag
        strobe_sum = np.sum(strobe_history)
        if strobe_sum == 0:
            # Skip current sample if NO strobe across N samples, i.e.,
            # strobe_history[1:] = [0, 0, ..., 0] && strobe_flag = 0
            pass
        elif strobe_sum == 1:
            # Shift TED buffer regularly if ONE strobe across N samples, i.e.,
            filter_interpolation_history = np.roll(filter_interpolation_history, -1)
            filter_interpolation_history[-1] = filter_interpolation_out
        else:
            # strobe_sum > 1
            # Stuff a missing sample if TWO strobes across N samples, i.e.,
            # strobe_history[1:] = [1, 0, ..., 0] && strobe_flag = 1
            filter_interpolation_history = np.roll(filter_interpolation_history, -2)
            filter_interpolation_history[-2] = 0
            filter_interpolation_history[-1] = filter_interpolation_out

        # Loop filter (PI)
        filter_loop_state += filter_loop_prev
        filter_loop_out = e * gain_proportional + filter_loop_state
        filter_loop_prev = e * gain_integrator

        # Interpolation controller
        # Modulo-1 counter interpolation controller which generates/updates strobe signal (strobe_flag)
        # and fractional interpolation interval (timing_error).
        # Refer to Section 8.4.3 and Figure 8.4.19 in Rice's book [1].
        W = filter_loop_out + 1 / samples_per_symbol  # W should be small when locked
        strobe_flag = (counter_val < W)  # Check if a strobe
        if strobe_flag:  # Upate timing_error if a strobe
            timing_error = counter_val / W
        counter_val = (counter_val - W) % 1  # Update counter

    return (samples_corrected[:strobe_count], timing_error_vec)


####################### End of Original Pipeline ############################


def sync_phase_and_everything(data, plot_data=False, fft_window_size=4096, samples_per_symbol=4, sampling_freq=4e6):
    normalized = pipeline_normalize_to_magnitude(data)
    if plot_data:
        plot_eye_cmp(data, normalized, samples_per_symbol=samples_per_symbol, s1_title='raw', s2_title='normalized',
                     disable_combo=False)

    filtered = matched_filter(normalized, samples_per_symbol=samples_per_symbol, )

    if plot_data:
        plot_eye_cmp(normalized, filtered, samples_per_symbol=samples_per_symbol, s1_title='normalized',
                     s2_title='filtered',
                     disable_combo=False)

    with_sync_frequency, frequency_offset = sync_freq(filtered, sampling_freq=sampling_freq,
                                                      fft_window_size=fft_window_size,
                                                      samples_per_symbol=samples_per_symbol)

    if plot_data:
        plot_eye_cmp(filtered, with_sync_frequency, samples_per_symbol=samples_per_symbol, s1_title='filtered',
                     s2_title='with_sync_frequency', disable_combo=False)

    samples_phase, phase_shift = sync_phase2(with_sync_frequency, samples_per_symbol=samples_per_symbol)

    if plot_data:
        plot_eye_cmp(with_sync_frequency, samples_phase, samples_per_symbol=samples_per_symbol,
                     s1_title='with_sync_frequency',
                     s2_title='phase sync', disable_combo=False)

    samples_phase2, samples_expected, time_offset, phase_ambiguity = sync_phase_time(samples_phase)

    if plot_data:
        plot_eye_cmp(samples_phase, samples_phase2, samples_per_symbol=samples_per_symbol, s1_title='phase sync',
                     s2_title='adjust for phase ambiguity', disable_combo=False)

    samples_time, time_error = sync_time(samples_phase2, samples_per_symbol=samples_per_symbol)

    if plot_data:
        plot_eye_cmp(samples_phase2, samples_time, samples_per_symbol=samples_per_symbol,
                     s1_title='adjust for phase ambiguity',
                     s2_title='sync time', disable_combo=False)

    avg_phase_shift:np.float32 = np.average(phase_shift)
    avg_time_error: np.float32 = np.average(time_error)
    # np.complex[], np.float32, np.float32, int, float , np.float32


    return samples_time, frequency_offset, avg_phase_shift, time_offset, phase_ambiguity, avg_time_error


chip_seqs: List[Tuple[
    int, int, int, int, int, int, int, int, int, int, int, int, int, int, int, int, int, int, int, int, int, int, int, int, int, int, int, int, int, int, int, int]] = \
    [
        (1, 1, 0, 1, 1, 0, 0, 1, 1, 1, 0, 0, 0, 0, 1, 1, 0, 1, 0, 1, 0, 0, 1, 0, 0, 0, 1, 0, 1, 1, 1, 0),
        (1, 1, 1, 0, 1, 1, 0, 1, 1, 0, 0, 1, 1, 1, 0, 0, 0, 0, 1, 1, 0, 1, 0, 1, 0, 0, 1, 0, 0, 0, 1, 0),
        (0, 0, 1, 0, 1, 1, 1, 0, 1, 1, 0, 1, 1, 0, 0, 1, 1, 1, 0, 0, 0, 0, 1, 1, 0, 1, 0, 1, 0, 0, 1, 0),
        (0, 0, 1, 0, 0, 0, 1, 0, 1, 1, 1, 0, 1, 1, 0, 1, 1, 0, 0, 1, 1, 1, 0, 0, 0, 0, 1, 1, 0, 1, 0, 1),
        (0, 1, 0, 1, 0, 0, 1, 0, 0, 0, 1, 0, 1, 1, 1, 0, 1, 1, 0, 1, 1, 0, 0, 1, 1, 1, 0, 0, 0, 0, 1, 1),
        (0, 0, 1, 1, 0, 1, 0, 1, 0, 0, 1, 0, 0, 0, 1, 0, 1, 1, 1, 0, 1, 1, 0, 1, 1, 0, 0, 1, 1, 1, 0, 0),
        (1, 1, 0, 0, 0, 0, 1, 1, 0, 1, 0, 1, 0, 0, 1, 0, 0, 0, 1, 0, 1, 1, 1, 0, 1, 1, 0, 1, 1, 0, 0, 1),
        (1, 0, 0, 1, 1, 1, 0, 0, 0, 0, 1, 1, 0, 1, 0, 1, 0, 0, 1, 0, 0, 0, 1, 0, 1, 1, 1, 0, 1, 1, 0, 1),
        (1, 0, 0, 0, 1, 1, 0, 0, 1, 0, 0, 1, 0, 1, 1, 0, 0, 0, 0, 0, 0, 1, 1, 1, 0, 1, 1, 1, 1, 0, 1, 1),
        (1, 0, 1, 1, 1, 0, 0, 0, 1, 1, 0, 0, 1, 0, 0, 1, 0, 1, 1, 0, 0, 0, 0, 0, 0, 1, 1, 1, 0, 1, 1, 1),
        (0, 1, 1, 1, 1, 0, 1, 1, 1, 0, 0, 0, 1, 1, 0, 0, 1, 0, 0, 1, 0, 1, 1, 0, 0, 0, 0, 0, 0, 1, 1, 1),
        (0, 1, 1, 1, 0, 1, 1, 1, 1, 0, 1, 1, 1, 0, 0, 0, 1, 1, 0, 0, 1, 0, 0, 1, 0, 1, 1, 0, 0, 0, 0, 0),
        (0, 0, 0, 0, 0, 1, 1, 1, 0, 1, 1, 1, 1, 0, 1, 1, 1, 0, 0, 0, 1, 1, 0, 0, 1, 0, 0, 1, 0, 1, 1, 0),
        (0, 1, 1, 0, 0, 0, 0, 0, 0, 1, 1, 1, 0, 1, 1, 1, 1, 0, 1, 1, 1, 0, 0, 0, 1, 1, 0, 0, 1, 0, 0, 1),
        (1, 0, 0, 1, 0, 1, 1, 0, 0, 0, 0, 0, 0, 1, 1, 1, 0, 1, 1, 1, 1, 0, 1, 1, 1, 0, 0, 0, 1, 1, 0, 0),
        (1, 1, 0, 0, 1, 0, 0, 1, 0, 1, 1, 0, 0, 0, 0, 0, 0, 1, 1, 1, 0, 1, 1, 1, 1, 0, 1, 1, 1, 0, 0, 0)
    ]

test_packet_half_bytes = [0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0xa, 0x7, 0x1, 0x1, 0x0, 0x1, 0x9, 0x8, 0x0, 0x1, 0x2,
                          0x2, 0x2,
                          0x2, 0x3, 0x4, 0x1, 0x2, 0x4, 0x4, 0x4, 0x4, 0xc, 0xd, 0xa, 0xb, 0x0, 0x0, 0x0, 0x1, 0x0, 0x2,
                          0x0, 0x3,
                          0x7, 0x7, 0x2, 0x7, ]

# reverse endianness b/c networking is weird
test_packet: List[int] = [v for sublist in [*zip(test_packet_half_bytes[1::2], test_packet_half_bytes[::2])] for v in
                          sublist]

preamble = test_packet[:8]


def verify_packet(
        samples_after_pipeline,
        expected_packet=test_packet,
        logging=False,
        first_4bits_of_header=0x0,
        chip_seqs=chip_seqs  # (ieee802154_chip_map + 1) * 2

):
    check_icon = "✅"
    x_icon = "❌"

    # 1 is same, 0 is least
    def similar(input_seq, chip_seq) -> float:
        # get the similarness of each chip sequence, 1 is same ... -1 is inverse
        c = cosine(input_seq, chip_seq)
        return 1 - abs(c)

    def get_most_similar_chip_seq(flattened_input_seq) -> (int, float):
        best = 0.0
        best_index = 0xee
        for (index, chip_seq) in enumerate(chip_seqs):
            current = similar(flattened_input_seq, chip_seq)
            if current > best:
                best = current
                best_index = index

        return best_index, best

    def point_to_chip(i: int, q: int) -> (int, int):
        if i < 0:
            i = 0
        if q < 0:
            q = 0
        return int(i), int(q)

    if logging:
        print("len(samples_time)", len(samples_after_pipeline))
    # find offset

    I = np.array([1 if x > 0 else -1 for x in np.real(samples_after_pipeline)])
    Q = np.round([1 if x > 0 else -1 for x in np.imag(samples_after_pipeline)])
    together = np.column_stack((I, Q))
    together = np.array([point_to_chip(i, q) for [i, q] in together])
    flat = together.flatten()
    start_index = -1
    for i in range(0, len(flat)):
        sublist = flat[i:i + 32]
        if np.array_equiv(sublist, np.array(list(chip_seqs[first_4bits_of_header]))):
            start_index = i
            break
    if start_index == -1:
        raise Exception("Failed to find start of packet")

    if logging:
        print("start_index", start_index)

    # together = np.array([[i, q] for [i, q] in together if i != 0.0 and q != 0.0])# remove 0s
    # together = np.array([point_to_chip(i, q) for [i, q] in together])

    # shortened_data_1 = together[int(start_index/2):]
    shortened_data = np.array([(flat[i], flat[i + 1]) for i in range(start_index, len(flat), 2)])

    possible_chip_seq = np.array_split(shortened_data, np.ceil(len(shortened_data) / 16))

    # ignore all bytes after the end of the test packet
    ret = True
    for i, test_byte in enumerate(expected_packet):
        # for (i, chip_seq) in enumerate(possible_chip_seq):
        chip_seq = possible_chip_seq[i]
        flat = chip_seq.flatten()
        (byte, diff) = get_most_similar_chip_seq(flattened_input_seq=flat)
        correct = byte == expected_packet[i]
        if not correct:
            ret = False
        if logging:
            if correct:
                print(f"{check_icon}{i}: {hex(byte)} ")
            else:
                print(f"{x_icon}{i}: act {hex(byte)} != exp {hex(expected_packet[i])}")
                print(f"\tin  -> {flat}")
                print(f"\texp -> {chip_seqs[expected_packet[i]]}")
        # if diff != 1:
        #     print(f"similarity: {diff}")
        #     print(f"\tin  -> {flat}")
        #     print(f"\tsim -> {chip_seqs[byte]}")

    return ret


def verify_packet_backoff(packet: Packet, logging=False, show_plots: bool = False,
                          fft_window_size: int = 4096, packet_len = 46 * 16) -> tuple[
    bool, int, tuple[NDArray[np.complex64], floating[_32Bit], floating[_32Bit], int, float, floating[_32Bit]]|None]:
    for i in range(0, 32):
        if logging:
            print("==> Backoff: ", i)

        if i == 0:
            data = packet.data
        else:
            data = packet.shift_start_back_by(i)[:-i]

        try:
            samples_time,  frequency_offset, avg_phase_shift, time_offset, phase_ambiguity, avg_time_error = sync_phase_and_everything(
                data,
                plot_data=False,
                fft_window_size=fft_window_size
            )
            valid = verify_packet(samples_time, logging=logging)
        except Exception as e:
            if logging:
                print("Exception: ", e)
            valid = False

        if valid:
            # crop the tail of the data to max packet size
            samples_time = samples_time[0:packet_len]

            if show_plots:
                sync_phase_and_everything(data, plot_data=True, fft_window_size=fft_window_size)
            return valid, i, (samples_time, frequency_offset, avg_phase_shift, time_offset, phase_ambiguity, avg_time_error)
    return False, -1, None


def extract_data() ->List[Tuple[
    bool, int, Tuple[NDArray[np.complex64], floating[_32Bit], floating[_32Bit], int, float, floating[_32Bit]]|None]]:

    total_data: NDArray[np.complex64] = np.load('../data_recording/DATA_8mhz.npy', 'r', allow_pickle=True, )[0]

    all_packets = find_packets(total_data)



    # for i, p in enumerate(all_packets):
    #     print(f"packet #{i}")
    #     (v, backoff, extra_data) = verify_packet_backoff(p)
    #     print(v, backoff)
    #     if not v:
    #         break


    extracted_packets = [verify_packet_backoff(p) for p in all_packets]
    return extracted_packets