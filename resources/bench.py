#
# generate benchmark test report.
#
# prerequisite:
# * install beautiful soap:
#   $ apt-get install python-bs4
#

import getopt
import sys
import os
import re
from bs4 import BeautifulSoup

def parse_log_file(log_file, report_file, rev):
    with open(log_file, 'r') as f:
        if not os.path.isfile(report_file):
            add_report_file(report_file)
        with open(report_file, "r") as rf:
            soup = BeautifulSoup(rf, "html.parser")

        any_bench_data = False
        for line in f.readlines():
            if "[benchtest_" in line:
                m = re.search("\[(\w+)\](.*)$", line)
                if m != None and  len(m.groups()) == 2:
                    print "processing line: " + line
                    print "test name: " + m.group(1)
                    cases = map(lambda x: x.strip(),re.split("[:,]", m.group(2)))
                    cases = zip(cases[::2], cases[1::2]);
                    for item in cases:
                        print "test desc: "+ item[0]
                        print "bench value: "+ item[1]
                    any_bench_data = True
                    update_data(m.group(1), cases, soup)
        if any_bench_data:
            sweep(soup, rev)
            #print(soup.prettify())
            with open(report_file, "w") as rf:
                rf.write(str(soup))
                #rf.write(soup.prettify())


def sweep(soup, rev):
    header_tr = soup.find('tr', id="header")
    th = soup.new_tag('th')
    th.string = rev
    header_tr.append(th)
    contents = [x for x in header_tr.contents if x.name =="th"]
    if len(contents) > 13:
        contents[3].decompose()

def update_data(testname, cases, soup):
    main_table = soup.find('table', id="main")
    expected_tr = soup.find('tr', id=testname)
    if expected_tr is not None:

        # sweep test case
        contents = [x for x in expected_tr.contents if x.name =="td"]

        java_td =contents[2]
        java_list = [x for x in java_td.contents if x.name == "table"]
        java_list = len(java_list) > 0 and [str(x) for x in java_list[0].contents if x.name == "tr"] or []
        java_list = len(java_list) > 0 and [re.search("<tr>\s*<td>(.+)</td>\s*</tr>", x).group(1) for x in java_list] or []
        table = soup.new_tag('table')
        table['class'] = "testcase"
        for i,item in enumerate(cases):
            case_tr = soup.new_tag('tr')
            case_td = soup.new_tag('td')
            case_td.string = item[1]
            if len(java_list) > 0 and float(java_list[i]) < float(item[1]):
                case_td['bgcolor'] = "red"
            case_tr.append(case_td)
            table.append(case_tr)
        data_td = soup.new_tag("td")
        data_td['class']="data"
        data_td.append(table)
        expected_tr.append(data_td)
        contents = [x for x in expected_tr.contents if x.name =="td"]

        if len(contents) > 13:
           contents[3].decompose()
    else:
        expected_tr = soup.new_tag('tr')
        expected_tr['id'] = testname
        # add name
        name_td = soup.new_tag('td')
        name_td['class'] = "name"
        name_td.string = testname
        expected_tr.append(name_td)

        # add desc
        desc_td = soup.new_tag('td')
        desc_td['class'] = 'desc'
        desc_table = soup.new_tag('table')
        desc_table['class'] = "testcase"
        for item in cases:
            case_tr = soup.new_tag('tr')
            case_td = soup.new_tag('td')
            case_td.string = item[0]
            case_tr.append(case_td)
            desc_table.append(case_tr)
        desc_td.append(desc_table)
        expected_tr.append(desc_td)

        #add empty data for java
        for i in range(0,1):
            data_td =soup.new_tag('td')
            data_td['class'] = 'data'
            expected_tr.append(data_td)

        #add data
        data_td = soup.new_tag("td")
        data_td['class']="data"
        data_table = soup.new_tag('table')
        data_table['class'] = "testcase"
        for item in cases:
            case_tr = soup.new_tag('tr')
            case_td = soup.new_tag('td')
            case_td.string = item[1]
            case_tr.append(case_td)
            data_table.append(case_tr)
        data_td.append(data_table)
        expected_tr.append(data_td)

        # add to table
        main_table.append(expected_tr)



def add_report_file(report_file):
    init_doc = """
<html><head><title>Bench Test Report</title></head>
<body>
<table id = "main" border="1">
<tr id="header"><th>Case Name</th><th>Case Desc</th><th>Java</th></tr>
</table>
</body>
</html>
"""
    with open(report_file, "w") as rf:
        rf.write(init_doc)


def usage():
    usage = """
Usage: python bench.sh [OPTION]...
  -l, --log        bench test log file path
  -r, --report     where to generate the report
  -c, --commit     git rev which bench test runs on
"""
    print usage

def main():
        try:
            opts, args = getopt.getopt(sys.argv[1:], "hl:r:c:", ["help", "log=", "report=", "commit="])
        except getopt.error, msg:
            print str(msg)
            usage()
            sys.exit(2)
        log_file = None
        report_file = None
        rev = None
        for opt, val in opts:
            if opt in ("-l", "--log"):
                if os.path.isfile(val):
                    log_file = val
                else:
                    print "log file doesn't exit."
                    usage()
                    sys.exit(2)
            elif opt in ("-r", "--report"):
                report_file = val
            elif opt in ("-c", "--commit"):
                rev = val
            elif opt in ("-h", "--help"):
                usage()
                sys.exit(0)
            else:
                print "invalid arguments"
                usage()
                sys.exit(2)
        if log_file is None or report_file is None or rev is None:
                print "invalid arguments"
                usage()
                sys.exit(2)
        parse_log_file(log_file, report_file, rev)

if __name__ == "__main__":
    main()
