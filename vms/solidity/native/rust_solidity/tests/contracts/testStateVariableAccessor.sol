pragma solidity ^0.4.8;

contract Test {
    mapping(address => mapping (address => uint)) public a;
    
    struct St {
		uint a;
		bytes32 b;
	}
    mapping(bytes5 => St) public b;
    
    uint8[] public c;
    
    address[] public d;
    
    function f() returns (uint) {
    	address a1 = 0x0102030405060708091011121314151617181920;
    	address a2 = 0x2122232425262728293031323334353637383940;
    	a[a1][a2] = 3;
    	
    	return a[a1][a2];
    }
    
    function g() {
    	St memory b1;
    	b1.a = 0x1;
    	b1.b = 0x2;
    	
    	b[0x3] = b1;
    }
    
    function h() returns (uint8) {
    	c.length = 2;
    	c[0x1] = 0x2;
    	
    	return c[0x1];
    }
    
    function i() returns (address) {
    	d.length = 2;
    	d[0x1] = 0x2;
    	
    	return d[0x1];
    }
}