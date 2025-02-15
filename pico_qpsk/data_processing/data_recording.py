import uhd
import numpy as np
import matplotlib.pyplot as plt

num_samples:int = 20_000_000  # number of samples received
center_freq = 2460e6 # 2.46GHz
sample_rate:float = 4e6 # 4MHz
gain = 50 # dB
usrp = uhd.usrp.MultiUSRP()
samples = usrp.recv_num_samps(num_samps=num_samples, freq=center_freq, rate=sample_rate,channels=(0,), gain=50) # units: N, Hz, Hz, list of channel IDs, dB
np.save("DATA_4mhz.npy", samples)

# print(samples[0:10])

# import uhd
# usrp = uhd.usrp.MultiUSRP()
# samples = usrp.recv_num_samps(8e6, 2460e6, 4e6, [0], 50)
print(samples[0:10])

