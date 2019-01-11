pragma solidity ^0.4.8;

contract Test {
    string public constant symbol = "FIXED";
    string public constant name = "Example Fixed Supply Token";
    uint8 public constant decimals = 18;
    uint _totalSupply = 1000000;

    // Owner of this contract
    address public owner;

    // Balances for each account
    mapping(address => uint) balances;

    // Owner of account approves the transfer of an amount to another account
    mapping(address => mapping (address => uint)) allowed;
}

