pragma solidity ^0.4.10;

contract Test {

    function f() returns (bytes32) {
        bytes32 x = 0x0102030405060708111213141516171821222324252627283132333435363738;
        return x << (24 * 8);
    }
    
    function g() returns (bytes32) {
        bytes32 x = 0x0102030405060708111213141516171821222324252627283132333435363738;
        return x << (16 * 8);
    }

    function h() returns (bytes32) {
        bytes32 x = 0x0102030405060708111213141516171821222324252627283132333435363738;
        return x << (8 * 8);
    }

    function i() returns (bytes32) {
        bytes32 x = 0x0102030405060708111213141516171821222324252627283132333435363738;
        return x >> (24 * 8);
    }
    
    function j() returns (bytes32) {
        bytes32 x = 0x0102030405060708111213141516171821222324252627283132333435363738;
        return x >> (16 * 8);
    }

    function k() returns (bytes32) {
        bytes32 x = 0x0102030405060708111213141516171821222324252627283132333435363738;
        return x >> (8 * 8);
    }
}
