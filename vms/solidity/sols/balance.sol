pragma solidity ^0.4.10;

contract Balance {

    uint id = 99;

    function get_id() public constant returns(uint) {
        return id;
    }

    function get_balance() public payable returns(uint) {
        return address(this).balance;
    }
}
