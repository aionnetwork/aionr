# key - ed25519

ed25519 key crate

### Usage

```
keys generator.
  Copyright (c) 2017-2018 Aion foundation.

Usage:
    keygen info <secret> [options]
    keygen generate [options]
    keygen sign <secret> <message>
    keygen verify public <public> <signature> <message>
    keygen verify address <address> <signature> <message>
    keygen [-h | --help]

Options:
    -h, --help         Display this message and exit.
    -s, --secret       Display only the secret.
    -p, --public       Display only the public.
    -a, --address      Display only the address.

Commands:
    info               Display public and address of the secret.
    generate           Generates new random key.
    sign               Sign message using secret.
    verify             Verify signer of the signature.
```

### Examples

#### `info <secret>`
*Display info about private key.*

- `<secret>` - secret, 32 bytes long

```
keygen info 17d08f5fe8c77af811caa0c9a187e668ce3b74a99acc3f6d976f075fa8e0be55
```

```
secret:  f4de4d6bd402652b28ac923001c02a6e213c97914b7ea88d072dc5e38a4284d4c58107da643895c3d52fce610a726ec531806968c567d98e43a9a769b134d048
public:  c58107da643895c3d52fce610a726ec531806968c567d98e43a9a769b134d048
address: a0bbeeef5046bbba157c37a13351a4eda0217866be63b2e4ac8c9b9b426f00e5
```

--


#### `generate`
*Generate new keypair randomly.*

```
keygen generate
```

```
secret:  f4de4d6bd402652b28ac923001c02a6e213c97914b7ea88d072dc5e38a4284d4c58107da643895c3d52fce610a726ec531806968c567d98e43a9a769b134d048
public:  c58107da643895c3d52fce610a726ec531806968c567d98e43a9a769b134d048
address: a0bbeeef5046bbba157c37a13351a4eda0217866be63b2e4ac8c9b9b426f00e5
```

--

#### `sign <secret> <message>`
*Sign a message with a secret.*

- `<secret>` - aion secret, 32 bytes long
- `<message>` - message to sign, 32 bytes long

```
keygen sign 17d08f5fe8c77af811caa0c9a187e668ce3b74a99acc3f6d976f075fa8e0be55 bd50b7370c3f96733b31744c6c45079e7ae6c8d299613246d28ebcef507ec987
```

```
4f288085aece4ce7ef3dce7fd44ea746833b32e308b4bf7165c56cce90a99d6335818c29f567d6df6d76acd8db43c1b5efcc9ab4b2dfac7063b34dce6a0612c5664d930122c10d9e4ad5253581b2b36a37bafef3ed43827f5e20d80f67fdb003
```

--

#### `verify public <public> <signature> <message>`
*Verify the signature.*

- `<public>` - aion public, 32 bytes long
- `<signature>` - message signature, 96 bytes long
- `<message>` - message, 32 bytes long

```
keygen verify public c58107da643895c3d52fce610a726ec531806968c567d98e43a9a769b134d048 8bc5c4e5599afac7cb0efcb0010540017dda3e80870bb543b356867b2a8cacbfe4b1f5ec4f0dae45abfddd1bd67d9eac51d291fa7d1ae0db10886323844180d36d3334b9232fded1f2fb9d010aa7c0f6e01d67355e9f5b417d680805f4051f0e bd50b7370c3f96733b31744c6c45079e7ae6c8d299613246d28ebcef507ec987
```

```
true
```

--

#### `verify address <address> <signature> <message>`
*Verify the signature.*

- `<address>` - aion address, 32 bytes long
- `<signature>` - message signature, 96 bytes long
- `<message>` - message, 32 bytes long

```
keygen verify address c58107da643895c3d52fce610a726ec531806968c567d98e43a9a769b134d048 8bc5c4e5599afac7cb0efcb0010540017dda3e80870bb543b356867b2a8cacbfe4b1f5ec4f0dae45abfddd1bd67d9eac51d291fa7d1ae0db10886323844180d36d3334b9232fded1f2fb9d010aa7c0f6e01d67355e9f5b417d680805f4051f0e 0000000000000000000000000000000000000000000000000000000000000001
```

```
true
```

--

