pragma solidity ^0.4.15;

contract BlockTransactionProps {
    
    bytes32 a = block.blockhash(1);
    address b = block.coinbase;
    uint c = block.difficulty;
    uint d = block.gaslimit;
    uint e = block.number;
    uint f = block.timestamp;
    bytes g = msg.data;
    uint h = msg.gas;
    address i = msg.sender;
    bytes4 j = msg.sig;
    uint k = msg.value;
    uint l = now;
    uint m = tx.gasprice;
    address n = tx.origin;
    
}

contract MathAndCrypto {
    uint a = addmod(1,2,3);
    uint b = mulmod(1,2,3);
    bytes32 c = keccak256(hex'11223344');
    bytes32 e = sha3(hex'11223344');
    
    // precompiled contracts
    bytes32 d = sha256(hex'11223344');
    bytes20 f = ripemd160(hex'11223344');
    address g = ecrecover(hex'11223344', 1, hex'11223344', hex'11223344');
}

contract AddressProps {
    
    function f() {
        address addr = 0x1234;
        addr.transfer(100);
        addr.send(100);
        addr.call(hex'11223344', 1);
        addr.callcode(hex'11223344', 2);
        addr.delegatecall(hex'11223344', 3);
        
        selfdestruct(addr);
    }
}
