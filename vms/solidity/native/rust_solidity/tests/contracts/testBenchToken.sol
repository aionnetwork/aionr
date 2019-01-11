pragma solidity ^0.4.8;

contract Token {

    event Transfer(address indexed _from, address indexed _to, uint _value);

    mapping(address => uint) balances;

    function mint(uint _amount) {
        balances[msg.sender] += _amount;
    }

    function transfer(address _to, uint _amount) returns (bool success) {
        if (balances[msg.sender] >= _amount
            && _amount > 0
            && balances[_to] + _amount > balances[_to]) {
            balances[msg.sender] -= _amount;
            balances[_to] += _amount;
            Transfer(msg.sender, _to, _amount);
            return true;
        } else {
            return false;
        }
    }
}
