pragma solidity ^0.4.0;

contract Precompiled {
    
    /**
     * From https://github.com/ethereum/go-ethereum/issues/3731
     */
    function testEcrecover() returns (address) {
    
        bytes32 msgHash = 0x852daa74cc3c31fe64542bb9b8764cfb91cc30f9acf9389071ffb44a9eefde46;
        bytes32 r = 0xb814eaab5953337fed2cf504a5b887cddd65a54b7429d7b191ff1331ca0726b1;
        bytes32 s = 0x264de2660d307112075c15f08ba9c25c9a0cc6f8119aff3e7efb0a942773abb0;
        uint8 v = 0x1b;
        
        bytes memory prefix = "\x19Ethereum Signed Message:\n32";
        bytes32 prefixedHash = sha3(prefix, msgHash);
                
        return ecrecover(prefixedHash, v, r, s);
    }

    function testSha256() returns (bytes32) {
        return sha256("hello");
    }
    
    function testRipemd160() returns (bytes20) {
        return ripemd160("hello");
    }
}