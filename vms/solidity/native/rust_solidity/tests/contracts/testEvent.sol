pragma solidity ^0.4.8;

contract Test {

    event Beer(address indexed from, bytes10 indexed note, uint amount);

    function f() {
        Beer(msg.sender, "test", 1);
    }
}