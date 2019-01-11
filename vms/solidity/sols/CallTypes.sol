pragma solidity ^0.4.0;

contract D {
    uint public n;
    address public sender;

    address e;
    function D() {
        e = new E();
    }

    function callSetN(uint _n) {
        e.call(bytes4(sha3("setN(uint128)")), _n); // E's storage is set, D is not modified
    }

    function callcodeSetN(uint _n) {
        e.callcode(bytes4(sha3("setN(uint128)")), _n); // D's storage is set, E is not modified
    }

    function delegatecallSetN(uint _n) {
        e.delegatecall(bytes4(sha3("setN(uint128)")), _n); // D's storage is set, E is not modified
    }
}

contract E {
    uint public n;
    address public sender;

    function setN(uint _n) {
        n = _n;
        sender = msg.sender;
    }
}