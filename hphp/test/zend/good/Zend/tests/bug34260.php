<?hh
class Faulty
{
    function __call($Method,$Args)
    {
        switch($Method)
        {
            case 'seg':
              echo "I hate me\n";
            break;
        }
    }

    function NormalMethod($Args)
    {
       echo "I heart me\n";
    }
}
<<__EntryPoint>> function main(): void {
$Faulty = new Faulty();
$Array = array('Some junk','Some other junk');

// This causes a seg fault.
$Failure = array_map(array($Faulty,'seg'),$Array);

// This does not.
$Failure = array_map(array($Faulty,'NormalMethod'),$Array);
}
