pragma solidity ^0.4.15;

contract Test {
    function g() {
    }

    function f() {
        address addr = this;
        addr.call(bytes4(keccak256("g()")));
    }
}
