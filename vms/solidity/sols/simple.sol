pragma solidity ^0.4.0;

contract Simple {

    uint a;

    event B(uint indexed b);

    function Test() {
    }

    function set(uint x) {
        a = x;
    }

    function get() constant returns (uint) {
        return a;
    }
}