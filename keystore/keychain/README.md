# keychain

Aion key management.

### Usage

```
Aion key management.
  Copyright (c) 2018-2019 Aion foundation.

Usage:
    keychain insert <secret> <password> [--dir DIR]
    keychain list [--dir DIR]
    keychain import [--src DIR] [--dir DIR]
    keychain remove <address> <password> [--dir DIR]
    keychain sign <address> <password> <message> [--dir DIR]
    keychain [-h | --help]

Options:
    -h, --help               Display this message and exit.
    --dir DIR                Specify the secret store directory. It may be either
                             aion, aion-(chain)
                             or a path [default: aion].
    --src DIR                Specify import source. It may be either
                             aion, aion-(chain)
                             or a path [default: aion].

Commands:
    insert             Save account with password.
    list               List accounts.
    import             Import accounts from src.
    remove             Remove account.
    sign               Sign message.
```

### Examples

#### `insert <secret> <password> [--dir DIR]`
*Encrypt secret with a password and save it in secret store.*

- `<secret>` - aion secret, 64 bytes long
- `<password>` - account password, file path
- `[--dir DIR]` - secret store directory, It may be either, aion, aion-(chain) or a path. default: aion

```
keychain insert 14e4613085865916e855c1fa86d17cbdf226b68860f8f0cdb1267136f4f36aac29d024758c2df66b622c8152c9f680d657793d1be25ccd331f2d56fc148ffce5 password.txt
```

```
0xa0d6adccfc2fbe18e1f40b9ca7c8cfedb737284387a0cc7d9d8d3f7afa1ebf2b
```

--

#### `list [--dir DIR]`
*List secret store accounts.*

- `[--dir DIR]` - secret store directory, It may be either, aion, aion-(chain) or a path. default: aion

```
keychain list
```

```
 0: 0xa05480f0440b5bfbf62dd9a8f9efe83a1dc00e9be3bb569d96e3958d048196cf
 1: 0xa07611e8110193fe44a44d249f67f11ff86e2067209a54edccb2ca0f5d8ea3e3
 2: 0xa00a2d0d10ce8a2ea47a76fbb935405df2a12b0e2bc932f188f84b5f16da9c2c
```

--

#### `import [--src DIR] [--dir DIR]`
*Import accounts from src.*

- `[--src DIR]` - secret store directory, It may be either, aion, aion-(chain) or a path. default: aion
- `[--dir DIR]` - secret store directory, It may be either, aion, aion-(chain) or a path. default: aion

```
keychain import
```

```
 0: 0xa05480f0440b5bfbf62dd9a8f9efe83a1dc00e9be3bb569d96e3958d048196cf
 1: 0xa07611e8110193fe44a44d249f67f11ff86e2067209a54edccb2ca0f5d8ea3e3
 2: 0xa00a2d0d10ce8a2ea47a76fbb935405df2a12b0e2bc932f188f84b5f16da9c2c
```

--

#### `remove <address> <password> [--dir DIR]`
*Remove account from secret store.*

- `<address>` - aion address, 32 bytes long
- `<password>` - account password, file path
- `[--dir DIR]` - secret store directory, It may be either, aion, aion-(chain) or a path. default: aion

```
keychain remove a05480f0440b5bfbf62dd9a8f9efe83a1dc00e9be3bb569d96e3958d048196cf password.txt
```

```
true
```

--

#### `sign <address> <password> <message> [--dir DIR]`
*Sign message with account's secret.*

- `<address>` - aion address, 32 bytes long
- `<password>` - account password, file path
- `<message>` - message to sign, 32 bytes long
- `[--dir DIR]` - secret store directory, It may be either, aion, aion-(chain) or a path. default: aion

```
keychain sign c0b60137639fbe63266fd021732aff7e489fc7342da792b4222a5be9ffc9d3ee4e5bd84ec2eb0ea3f2ff14b3df20587d2bc78edde34cf315766d3c7f147f2c5d password.txt 0000000000000000000000000000000000000000000000000000000000000001
```

```
0x8bc5c4e5599afac7cb0efcb0010540017dda3e80870bb543b356867b2a8cacbfe4b1f5ec4f0dae45abfddd1bd67d9eac51d291fa7d1ae0db10886323844180d36d3334b9232fded1f2fb9d010aa7c0f6e01d67355e9f5b417d680805f4051f0e
```

--
