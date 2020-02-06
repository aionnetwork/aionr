pragma solidity ^0.4.8;

contract Test {
    function f() returns (bytes32) {
        return sha3("hello");
    }
    
    function g() returns (bytes32) {
        return sha256("hello");
    }
}