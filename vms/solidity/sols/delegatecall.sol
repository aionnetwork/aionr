pragma solidity ^0.4.10;

contract D {
  uint public n;
  address public sender;

  function callSetN(address _e, uint _n) {
    _e.call(bytes4(sha3("setN(uint256)")), _n); // E's storage is set, D is not modified 
  }

  function callcodeSetN(address _e, uint _n) {
    _e.callcode(bytes4(sha3("setN(uint256)")), _n); // D's storage is set, E is not modified 
  }

  function delegatecallSetN(address _e, uint _n) {
    _e.delegatecall(bytes4(sha3("setN(uint256)")), _n); // D's storage is set, E is not modified 
  }
}

contract E {
  uint public n;
  address public sender;

  function setN(uint _n) {
    n = _n;
    sender = msg.sender;
    // msg.sender is D if invoked by D's callcodeSetN. None of E's storage is updated
    // msg.sender is C if invoked by C.foo(). None of E's storage is updated

    // the value of "this" is D, when invoked by either D's callcodeSetN or C.foo()
  }
}

contract C {
    function foo(D _d, E _e, uint _n) {
        _d.delegatecallSetN(_e, _n);
    }
}