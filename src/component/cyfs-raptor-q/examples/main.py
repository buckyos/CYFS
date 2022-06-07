import os
import random
from raptorq import Encoder, Decoder


def main():
    # Generate some random data to send
    data = os.urandom(10000)

    # Create the Encoder, with an MTU of 1400 (common for Ethernet)
    encoder = Encoder.with_defaults(data, 1400)

    # Perform the encoding, and serialize to bytes for transmission
    packets = encoder.get_encoded_packets(15)

    # Here we simulate losing 10 of the packets randomly. Normally, you would send them over
    # (potentially lossy) network here.
    random.shuffle(packets)
    # Erase 10 packets at random
    packets = packets[:-10]

    # The Decoder MUST be constructed with the configuration of the Encoder.
    # The configuration should be transmitted over a reliable channel
    decoder = Decoder.with_defaults(len(data), 1400)

    # Perform the decoding
    result = None
    for packet in packets:
        result = decoder.decode(packet)
        if result is not None:
            break

    # Check that even though some of the data was lost we are able to reconstruct the original message
    assert result == data


if __name__ == '__main__':
    main()
