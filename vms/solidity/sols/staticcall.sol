pragma solidity ^0.4.10;

contract static_call {
    address owner;

    function static_call() {
        owner = msg.sender;
    }

    function my_call() constant returns (uint) {
        return 1;
    }

    function trigger() returns (uint) {
        return my_call();
    }
}