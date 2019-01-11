pragma solidity ^0.4.10;

contract view_test {
    function A() constant {
        int data = 10;
    }

    function B() {
        A();
    }
}