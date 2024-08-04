# SMDupeRemover
 CLI tool to find and remove duplicate filenames in Soundminer SQLITE databases.  

### USE AT YOUR OWN RISK I OFFER NO SUPPORT OF ANY KIND
### THIS LITERALLY DELETES RECORDS FROM YOUR DATABASE, SO I ADVISE YOU NEVER USE IT UNLESS YOU ARE VERY CERTAIN

Usage: 
    SMDupeRemover <database> arguments

The main program will Copy your database to a new database with _thinned added for identification.  It will then scan the new database for identical filenames and then use logic to decide which one to keep.

The default comparison logic is:
   Duration
   Channels
   Sample Rate
   Bit Rate
   BWdate (original file creation date)
   Date Added to Database

you can change this by creating a file called order.txt  or adding --generate-config-files as an argument

You can also COMPARE two databases and remove any file in the main database that also exists in the comparison database

You can also optionally have it scan for a series of characters (tags) and remove any files with them.  This is useful for finding all those -PiSH_ and -RVRS_ files protools generates.  There is an included default list, or you can create your own tags.txt.  Again --geneerate-config-files will create these files showing you the default list


## ARGUMENTS:

### -g or --generate-config-files
generates tags.txt and order.txt. order.txt defines how the main duplicate checker decides which file to keep.  this will overwrite what's there with the default values if they already exist in the directory.  I suggest running once and then modifying from there if you like.  Without these files, the program will just do the default order/tags I have pre programmed in the program.

### -c or --compare <database2>
if any file in the target database exists in the comparison database, it will be removed

### -p or --prune-tags
looks for common Protools Processing Tags and removes files with them.  can use tags.txt to define them.

### -n or --no-filename-check
this doesn't run the normal duplicate filename check on the database.  Useful if you want to just remove tags or compare with another database only.

### -d or --create-duplicates-database
after processing the database it will generate a new database containing all the deletions that were made

### -v or --verbose
displays each file as it's being deleted.  Can FLOOD your terminal

### -y or --no-prompt
automatically responds yes to the processing prompts

### -u or --unsafe
writes DIRECTLY to the database.  Also, skips all the yes, no warnings.  USE WITH CAUTION.

### -h or --help
gives a nice help summary

    

The Program runs in the following order if all optional flags are enabled:
  Compare Database, then Check For Duplicates in the main database, then Prune Tags.
