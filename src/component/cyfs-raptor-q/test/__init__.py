import os
import random
from unittest import TestCase


class EncoderDecoderTestCase(TestCase):
    def test_encoder(self):
        from raptorq import Encoder

        data = os.urandom(1024)
        encoder = Encoder.with_defaults(data, 512)
        packets = encoder.get_encoded_packets(42)

        self.assertIsInstance(packets, list)
        self.assertGreater(len(packets), 0)
        for packet in packets:
            self.assertIsInstance(packet, bytes)

    def test_decoder(self):
        from raptorq import Encoder, Decoder

        data = os.urandom(1024)
        encoder = Encoder.with_defaults(data, 512)
        packets = encoder.get_encoded_packets(42)

        random.shuffle(packets)

        decoded_data = None
        decoder = Decoder.with_defaults(len(data), 512)
        for packet in packets:
            decoded_data = decoder.decode(packet)
            if decoded_data is not None:
                break

        self.assertEqual(decoded_data, data)
