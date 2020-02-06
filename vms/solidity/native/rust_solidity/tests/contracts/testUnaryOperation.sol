pragma solidity ^0.4.8;

contract Test {
    function f() returns (bytes32) {
        bytes32 x = 0x0102030405060708091011121314151617181920212223242526272829303132;
        
        return ~x;
    }
}