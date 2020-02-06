pragma solidity ^0.4.10;

contract Test {
    mapping(address => uint) public balances;
    
    function f() {
        balances[msg.sender] = 1;
    }
}