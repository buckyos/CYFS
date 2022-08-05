# Package box
In order to embed multiple protocol packages in a link layer packet, we introduce desgin of package box. A package box corresponds to a packet on the link layer:
+ In udp, a package box is encoded into a udp datagram;
+ In tcp, we define the package box boundary with which package boxes can be splited from streamed data.

The most common scenario is: connecting a `Stream` for the first time between a pair of devices(call thems D1 and D2), D1 and D2 can be connected by a udp link. For connecting a `Tunnel` occurs at same time, connecting a `Stream` and connecting a `Tunnel` have their own independent three handshakes, 6 datagrams sent:
```
udp datagram 1: D1 -> D2, Tunnel Sync
udp datagram 2: D2 -> D1, Tunnel Ack
udp datagram 3: D1 -> D2, Tunnel AckAck
udp datagram 4: D1 -> D2, Stream Sync 
udp datagram 5: D2 -> D1, Stream Ack
udp datagram 6: D1 -> D2, Stream AckAck
```
If we can put handshake packets in one udp datagram, the above process actually becomes:
```
udp datagram 1: D1 -> D2, Tunnel Sync and Stream Sync 
udp datagram 2: D2 -> D1, Tunnel Ack and Stream Ack
udp datagram 3: D1 -> D2, Tunnel AckAck and Stream AckAck
```

# Encoding of a package
We design encoding format of a single protocol package as follows:
```
type code | flags | fields
```

+ type code: 8 bits, identifies the protocol package type;
+ flags: 16 bits, each bit can be used to mark a switch value, generally used to mark whether a field value is encoded;
+ fields: encode package's fields

# Package merging
Embeding multiple protocol packages in one package box also introduces an opportunity: if the protocol package embeded into a same package box have same fields. The value is only actually encoded once, marked by a smaller flag bit to mark these fields refer to this value.

Maximum code length of the package box is limited by the MTU of the link layer. The code length of the package box can be greatly reduced by package merging.

After introducing package merging, the encoding process for a array of packages `[package...]` P[] is as follows: 
1. For the first package P[0], encode each field;
2. For each mergeable field F of the subsequent package P[N], 
+ if P[N][F]==P[0][F], set the corresponding flag bit of F to 0, skip it; 
+ otherwise, set the corresponding flag bit to 1, encode it;

Correspondingly, the decoding process of P[] is as follows: 
1. Decode each field value of the first package P[0];
2. For each mergeable field F of the subsequent package P[N], 
+ if the corresponding flag bit is 0, set it to P[0][F]; 
+ otherwise, decode it;

# Exchange package
The package box should also be encrypted. The device object naturally contains a pair of asymmetric encryption keys. Encrypting all protocol packages with the remote's public key on `Tunnel` can certainly meet the requirements, but this is obviously inefficient.Similar to TLS, generate a random symmetric key and use asymmetric key  to securely notify remote of the key, then use the symmetric key to encrypt subsequent packages. We define Exchange package to deliver a new symmetric key.

```rust
pub struct Exchange {
    pub sequence: TempSeq,
    pub seq_key_sign: Signature,
    pub from_device_id: DeviceId,
    pub send_time: Timestamp,
    pub from_device_desc: Device,
}
```
Exchange package should always be encoded in the first package of the package box. A package box containing a exchange package is encoded as follows:
```
Public key encrypted symmetric key | Exchange Package | [package, ...]
```
1. Local device generates a new random symmetric key K, defines a bidirectional mapping data structure M[device id <-> key], set M[remote device id]=K;
2. Encrypt K with remove device's public key, write it into the header of the package box;
3. Generate an incremental Exchange.sequence value;
4. Set Exchange.seq_key_sign = signature of K and Exchange.sequence with local device's secret;
5. Encode [Exchange, package, ...] with package merging, and encrypt it with K;

Correspondingly, when remote receives a package box containing a Exchange package,  the process of decoding is as follows:
1. Decrypt K with local device's secret;
2. Decode [Exchange, package, ...] with K;
3. Verify K and Exchange.sequece with remove device's public key;
4. Set M[remote device id]=K;

After delivering K to remote device, when sending subsequent package boxes from same `Tunnel`, use K to encryt them, and no need exchange package.

A package box encrypted with K is encoded as follows:
```
hash(K) | [package, ...]
```
1. Using a hash algorithm agreed by both devices, write hash(K) into the header of the package box;
2. Encode [package, ...] with package merging, and encrypt it with K;

Correspondingly, when remote receives a package box containing hash(K), the process of decoding is as follows:
1. Search hash(K) in bidirectional mapping data structure M[device id <-> key], got key and device id;
2. Decode [package, ...] with K;

# Plaintext package box
From the overall design requirements of BDT, the connection process of `Tunnel` should be fully encrypted，a symmetric key exchange is always done.
We can use first bit of hash(K) in package box to mark whether a package box is encrypted, hash' is defined as: hash'(K) = hash(K) & 0x7f, making first bit of hash'(K) is 0.

A plaintext package box on a `Tunnel` encrypted with K is encoded as follows:
1. Write hash'(K) into the header of the package box;
2. Encode [package, ...] with package merging;
3. Set first bit of package box to 1;

# Decode package box
Combining all the above situations, the process of decoding a package box from a piece of data D is as follows:
1. Read first sizeof(hash) bytes from D, as H;
2. Let H' = H & 0x7f, making first bit of H' is 0; 
3. Search H' in bidirectional mapping data structure M[device id <-> key]:
+ If a key got, as K, if first bit of H is 0, decode [package, ...] with K; otherwise decode plaintext [package, ...];
+ Otherwise, try decrypt K with local device's secret, decode [Exchange, package, ...] with K;

# Package box on tcp stream
We simply define a signed int 16 header to split package box on a tcp stream, the value of header is set to encoding length of the packag box; the process of decoding package boxes from a tcp stream is as follows:
1. Read 2 bytes int 16 value, as H;
2. Read |H| bytes, as D;
3. Decode a package box from D;

When a tcp stream is used as tcp tunnel, the first package box sent on the tcp stream should have the complete encoding defined above to deliver a key, as K; then use this key K to encrypt subsequent package box on this tcp stream, hash'(K) on packag box header is ignored, we still use first bit of package box as encryption marker, that is the sign bit of the signed int 16 header. The process of decoding subsequent package boxes from a tcp stream is as follows:
1. Read 2 bytes int 16 value, as H;
2. Read |H| bytes, as D;
+ If H > 0, decode [package, ...] from D with K;
+ If H < 0, decode plaintext [package, ...] from D;