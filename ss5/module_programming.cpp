#include "module_programming.h"
#include <QFile>

namespace {
quint16 le16(const QByteArray &d,int p) {
    return quint16(quint8(d.at(p)))|(quint16(quint8(d.at(p+1)))<<8);
}
void be16(QByteArray *d,quint16 v) { d->append(char(v>>8)); d->append(char(v)); }
void addCrc(QByteArray *d) {
    quint16 c=ModuleProgramming::crc16(*d); d->append(char(c)); d->append(char(c>>8));
}
bool station(QByteArray *d,quint16 s) {
    if(s<=247){d->append(char(s));return true;}
    if(s<248||s>2295)return false;
    s-=248; d->append(char(0xF8|((s>>8)&7))); d->append(char(s)); return true;
}
int foff(const QByteArray &f) {
    return f.isEmpty()?-1:((quint8(f.at(0))&0xF8)==0xF8?2:1);
}
bool responseOk(const QByteArray &f,quint8 expected,QString *e) {
    int o=foff(f);
    if(o<0||f.size()<o+3){if(e)*e=QString::fromUtf8("Короткий ответ");return false;}
    quint16 got=quint8(f.at(f.size()-2))|(quint16(quint8(f.at(f.size()-1)))<<8);
    if(got!=ModuleProgramming::crc16(f.constData(),f.size()-2)){
        if(e)*e=QString::fromUtf8("Ошибка CRC ответа");return false;
    }
    quint8 fn=quint8(f.at(o));
    if(fn==quint8(expected|0x80)){
        if(e)*e=QString::fromUtf8("Modbus Exception %1").arg(quint8(f.at(o+1)));
        return false;
    }
    if(fn!=expected){if(e)*e=QString::fromUtf8("Неожиданная функция %1").arg(fn);return false;}
    return true;
}
}

namespace ModuleProgramming {
quint16 crc16(const char *data,int size,quint16 crc) {
    for(int i=0;i<size;i++){crc^=quint8(data[i]);for(int b=0;b<8;b++)
        crc=(crc&1)?quint16((crc>>1)^0xA001):quint16(crc>>1);}
    return crc;
}
quint16 crc16(const QByteArray &d){return crc16(d.constData(),d.size());}

bool inspectImage(const QByteArray &im,int slot,ImageInfo *out,QString *e) {
    if(slot<0||slot>=SlotCount){if(e)*e=QString::fromUtf8("Слот должен быть 0..19");return false;}
    if(im.size()<HeaderSize||im.size()>SlotSize){
        if(e)*e=QString::fromUtf8("Размер файла должен быть 10..4096 байт");return false;
    }
    ImageInfo x; x.storedCrc=le16(im,0); x.entryOffset=le16(im,2);
    x.bodySize=le16(im,4); x.type=le16(im,6); x.version=le16(im,8);
    if(x.entryOffset<HeaderSize||x.entryOffset>=SlotSize||x.entryOffset>=im.size()){
        if(e)*e=QString::fromUtf8("Неверное смещение точки входа");return false;
    }
    quint32 body=x.bodySize?x.bodySize:quint32(SlotSize-x.entryOffset);
    x.protectedEnd=quint32(x.entryOffset)+body;
    if(!body||x.protectedEnd>SlotSize||(x.bodySize&&x.protectedEnd!=quint32(im.size()))){
        if(e)*e=QString::fromUtf8("Неверный размер тела модуля");return false;
    }
    x.calculatedCrc=crc16(im.constData()+2,im.size()-2);
    char ff=char(0xFF);
    if(!x.bodySize)for(quint32 p=im.size();p<x.protectedEnd;p++)
        x.calculatedCrc=crc16(&ff,1,x.calculatedCrc);
    if(x.storedCrc&&x.storedCrc!=x.calculatedCrc){
        if(e)*e=QString::fromUtf8("CRC файла %1, вычислено %2")
            .arg(x.storedCrc,4,16,QChar('0')).arg(x.calculatedCrc,4,16,QChar('0'));
        return false;
    }
    if(out)*out=x; if(e)e->clear(); return true;
}
bool loadImage(const QString &name,int slot,QByteArray *image,ImageInfo *info,QString *e){
    QFile f(name); if(!f.open(QIODevice::ReadOnly)){
        if(e)*e=QString::fromUtf8("Не удалось открыть %1: %2").arg(name).arg(f.errorString());
        return false;
    }
    QByteArray d=f.readAll(); if(!inspectImage(d,slot,info,e))return false;
    if(image)*image=d; return true;
}
QVector<quint16> imageWords(const QByteArray &im){
    QVector<quint16> w; w.reserve((im.size()+1)/2);
    for(int p=0;p<im.size();p+=2){quint16 hi=quint8(im.at(p));
        quint16 lo=p+1<im.size()?quint8(im.at(p+1)):0xFF;
        w.append(quint16((hi<<8)|lo));} return w;
}
QByteArray makeReadHoldingFrame(quint16 a,quint16 n,quint16 s){
    QByteArray f;if(!n||n>125||!station(&f,s))return QByteArray();
    f.append(char(3));be16(&f,a);be16(&f,n);addCrc(&f);return f;
}
QByteArray makeWriteSingleFrame(quint16 a,quint16 v,quint16 s){
    QByteArray f;if(!station(&f,s))return f;f.append(char(6));
    be16(&f,a);be16(&f,v);addCrc(&f);return f;
}
QByteArray makeWriteMultipleFrame(quint16 a,const QVector<quint16>&v,quint16 s){
    QByteArray f;if(v.isEmpty()||v.size()>MaxWriteRegisters||!station(&f,s))return f;
    f.append(char(16));be16(&f,a);be16(&f,quint16(v.size()));f.append(char(v.size()*2));
    for(int i=0;i<v.size();i++)be16(&f,v.at(i));addCrc(&f);return f;
}
QList<QByteArray> makeDataWriteFrames(const QByteArray &im,quint16 s,int n){
    QList<QByteArray> r;if(n<1||n>MaxWriteRegisters||im.size()<HeaderSize||im.size()>SlotSize)return r;
    QVector<quint16>w=imageWords(im);
    for(int p=0;p<w.size();){int count=qMin(n,w.size()-p);
        r.append(makeWriteMultipleFrame(quint16(DataBase+p),w.mid(p,count),s));p+=count;}
    return r;
}
QByteArray makeSelectFrame(quint16 slot,quint16 len,quint16 s){
    QVector<quint16>v;v<<slot<<len;return makeWriteMultipleFrame(SlotRegister,v,s);
}
QByteArray makeVerifyFrame(quint16 s){return makeWriteSingleFrame(CommandRegister,CommandVerify,s);}
QByteArray makeStatusReadFrame(quint16 s){return makeReadHoldingFrame(StatusRegister,5,s);}
QByteArray makeConfirmFrame(quint16 t,quint16 s){return makeWriteSingleFrame(ConfirmTokenRegister,t,s);}
QByteArray makeWriteFrame(quint16 s){return makeWriteSingleFrame(CommandRegister,CommandWrite,s);}
QByteArray makeStartFrame(quint16 s){return makeWriteSingleFrame(CommandRegister,CommandStart,s);}
QByteArray makeStopFrame(quint16 s){return makeWriteSingleFrame(CommandRegister,CommandStop,s);}

bool parseReadHoldingResponse(const QByteArray &f,QVector<quint16>*values,QString*e){
    if(!responseOk(f,3,e))return false;int o=foff(f),pos,bytes;
    if(o==2){if(f.size()<7)return false;bytes=(quint8(f.at(o+1))<<8)|quint8(f.at(o+2));pos=o+3;}
    else{bytes=quint8(f.at(o+1));pos=o+2;}
    if((bytes&1)||f.size()!=pos+bytes+2){
        if(e)*e=QString::fromUtf8("Неверная длина FC03");return false;
    }
    if(values){values->clear();for(int i=0;i<bytes;i+=2)
        values->append(quint16((quint16(quint8(f.at(pos+i)))<<8)|quint8(f.at(pos+i+1))));}
    if(e)e->clear();return true;
}
bool parseOperationStatus(const QByteArray &f,OperationStatus*s,QString*e){
    QVector<quint16>v;if(!parseReadHoldingResponse(f,&v,e))return false;
    if(v.size()!=5){if(e)*e=QString::fromUtf8("Ожидалось 5 слов состояния");return false;}
    if(s){s->status=v[0];s->result=v[1];s->crc=v[2];s->slot=v[3];s->length=v[4];}
    return true;
}
bool isWriteAcknowledge(const QByteArray&q,const QByteArray&r,QString*e){
    int qo=foff(q);if(qo<0||q.size()<qo+7)return false;quint8 fn=quint8(q.at(qo));
    if((fn!=6&&fn!=16)||!responseOk(r,fn,e))return false;int ro=foff(r);
    if(r.size()!=ro+7||r.mid(ro,5)!=q.mid(qo,5)){
        if(e)*e=QString::fromUtf8("Ответ записи не совпадает с запросом");return false;}
    if(e)e->clear();return true;
}
quint16 expectedToken(quint16 c,quint16 slot,quint16 len){
    quint16 t=quint16(c^quint16(slot<<8)^len^0x5AA5);return t?t:0xA55A;
}
QString resultText(quint16 r){
    switch(r){case 0:return QString::fromUtf8("успешно");case 0xFFFF:return QString::fromUtf8("неверный слот");
    case 0xFFFE:return QString::fromUtf8("неверный заголовок");case 0xFFFD:return QString::fromUtf8("неверный размер");
    case 0xFFFC:return QString::fromUtf8("ошибка CRC/token");case 0xFFFB:return QString::fromUtf8("версия запрещена");
    case 0xFFFA:return QString::fromUtf8("ошибка W25Q128");case 0xFFF8:return QString::fromUtf8("ошибка потока");
    case 0xFFF7:return QString::fromUtf8("занято");default:return QString("0x%1").arg(r,4,16,QChar('0'));}
}
QString statusText(quint16 s){
    switch(s){case 0:return QString::fromUtf8("ожидание");case 1:return QString::fromUtf8("проверка");
    case 2:return QString::fromUtf8("проверено");case 3:return QString::fromUtf8("запись");
    case 4:return QString::fromUtf8("записано");case 5:return QString::fromUtf8("ошибка");
    case 6:return QString::fromUtf8("запуск");case 7:return QString::fromUtf8("работает");
    case 8:return QString::fromUtf8("остановка");case 9:return QString::fromUtf8("остановлен");
    default:return QString::number(s);}
}
}
